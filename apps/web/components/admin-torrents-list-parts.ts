"use client";

/**
 * 后台种子管理·复核对话 + 多维筛选状态（从 admin-torrents-list.tsx 拆出，
 * 守 300 行门禁）。两件事都是「自带 IO 的逻辑」而非渲染：
 *   · `useDecide`：通过/拒绝种子（拒绝时拉拒绝原因列表走 prompt）
 *   · `useDims`：自建维度筛选的取值集合 + 维度元数据/字典抓取
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { DenyReason } from "./admin-torrents-shared";
import type {
  SectionDictRow,
  SectionKindMeta,
  ValueMap,
} from "./admin-torrents-dims";

/** 审核裁决：approve=true 直接通过；false 时先选/填拒绝原因 */
export function useDecide(flash: (m: string) => void, reload: () => void) {
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  return useCallback(
    async (id: number, approve: boolean) => {
      let deny_reason_id: number | undefined;
      let reason = "";
      if (!approve) {
        const rs: DenyReason[] = await api.get("/api/v1/admin/deny-reasons");
        const list = rs.map((r) => `${r.id}. ${r.reason}`).join("\n");
        const choice = prompt(fmt(at.denyPrompt, { list }));
        if (!choice) return;
        const n = Number(choice);
        if (n > 0 && rs.some((r) => r.id === n)) deny_reason_id = n;
        else reason = choice;
      }
      try {
        await api.post("/api/v1/admin/reviews/decide", {
          torrent_id: id,
          approve,
          reason,
          deny_reason_id,
        });
        flash(fmt(approve ? at.approvedMsg : at.rejectedMsg, { id }));
        reload();
      } catch (e) {
        flash(e instanceof ApiError ? e.message : at.opFail);
      }
    },
    [at, flash, reload],
  );
}

/**
 * 自建维度筛选状态。维度/字典一次拉取（管理端接口），取值放本地 state。
 * 只保留「启用中且挂了字典项或本就无需字典」的维度——没有可选值的枚举维度
 * 渲染出来是个空框，不如不显示。
 */
export function useDims() {
  const [kinds, setKinds] = useState<SectionKindMeta[]>([]);
  const [dict, setDict] = useState<Record<string, SectionDictRow[]>>({});
  const [values, setValues] = useState<ValueMap>({});
  useEffect(() => {
    api
      .get<SectionKindMeta[]>("/api/v1/admin/section-kinds")
      .then(setKinds)
      .catch(() => {});
    api
      .get<SectionDictRow[]>("/api/v1/admin/section-dict")
      .then((rows) => {
        const m: Record<string, SectionDictRow[]> = {};
        for (const r of rows) (m[r.kind] ??= []).push(r);
        setDict(m);
      })
      .catch(() => {});
  }, []);
  const set = useCallback((key: string, value: string) => {
    setValues((prev) => ({ ...prev, [key]: value }));
  }, []);
  const clear = useCallback(() => setValues({}), []);
  // 只显示启用中、且有可选值的维度：停用维度不该出现在筛选器里；
  // 枚举维度没有字典项时渲染出来是个空框，同样不如不显示。
  const visible = kinds.filter((k) => {
    if (k.enabled === false) return false;
    const isEnum =
      k.field_type === "select" || k.field_type === "multiselect";
    return !isEnum || (dict[k.kind]?.length ?? 0) > 0;
  });
  return { kinds: visible, dict, values, set, clear };
}

/** 把取值集合写进查询串（空值跳过） */
export function appendDims(params: URLSearchParams, values: ValueMap) {
  for (const [k, v] of Object.entries(values)) {
    if (v.trim() !== "") params.set(k, v.trim());
  }
}
