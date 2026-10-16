"use client";

/**
 * 后台种子管理·复核对话 + 多维筛选状态（从 admin-torrents-list.tsx 拆出，
 * 守 300 行门禁）。两件事都自带 IO；useDecide 另持有拒绝弹窗的状态，调用方
 * 把返回的 denyDialog 渲染进 JSX——此前它能保持纯逻辑，是因为拒绝走的是浏览
 * 器原生 prompt，而那条路被 DS §3.1 判退了。
 *   · `useDecide`：通过/拒绝种子（拒绝走理由弹窗，不再用原生 prompt）
 *   · `useDims`：自建维度筛选的取值集合 + 维度元数据/字典抓取
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { AdminDenyDialog } from "./admin-deny-dialog";
import type {
  SectionDictRow,
  SectionKindMeta,
  ValueMap,
} from "./admin-torrents-dims";

/** 审核裁决：approve=true 直接通过；false 打开理由弹窗（AdminDenyDialog）。
 *  弹窗状态挂在这，调用方把返回的 denyDialog 渲染进 JSX。 */
export function useDecide(flash: (m: string) => void, reload: () => void) {
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  const [denyFor, setDenyFor] = useState<number | null>(null);
  const decide = useCallback(
    async (id: number, approve: boolean) => {
      if (!approve) {
        setDenyFor(id);
        return;
      }
      try {
        await api.post("/api/v1/admin/reviews/decide", {
          torrent_id: id,
          approve: true,
        });
        flash(fmt(at.approvedMsg, { id }));
        reload();
      } catch (e) {
        flash(e instanceof ApiError ? e.message : at.opFail);
      }
    },
    [at, flash, reload],
  );
  const denyDialog = (
    <AdminDenyDialog
      open={denyFor !== null}
      torrentId={denyFor ?? 0}
      onClose={() => setDenyFor(null)}
      onDone={(msg) => {
        setDenyFor(null);
        flash(msg);
        reload();
      }}
    />
  );
  return { decide, denyDialog };
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
