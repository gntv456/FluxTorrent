"use client";

import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";
import type { SectionKindMeta } from "@/components/admin-sections-shared";

/** 编辑提交载荷 + 补种/删除动作（0184 从 torrent-manage.tsx 拆出，
 *  300 行门禁）：buildEditPayload 组 PUT /torrents/{id} body（含推荐位
 *  staff 分支）；useTorrentActions 封装 reseed/del 的 API 调用与提示。 */

/** 补种/删除动作（0184 从 torrent-manage.tsx 拆出，300 行门禁）：
 *  reseed = PM 全体完成者（900s 限频后端校验）；del = 软删 + 跳列表 */
export function useTorrentActions(opts: {
  torrentId: number;
  setMsg: (m: string | null) => void;
  setBusy: (b: boolean) => void;
  onDeleted: () => void;
}) {
  const { dict } = useI18n();
  async function reseed() {
    opts.setBusy(true);
    opts.setMsg(null);
    try {
      const r = await api.post<{ notified: number }>(
        `/api/v1/torrents/${opts.torrentId}/reseed`,
        {},
      );
      opts.setMsg(dict.reseed2.ok.replace("{n}", String(r.notified)));
    } catch (e) {
      opts.setMsg(
        e instanceof ApiError ? e.message : dict.common.networkError,
      );
    } finally {
      opts.setBusy(false);
    }
  }
  async function del() {
    if (!window.confirm(dict.torrentManage2.delConfirm)) return;
    opts.setBusy(true);
    opts.setMsg(null);
    try {
      await api.del(`/api/v1/torrents/${opts.torrentId}`);
      opts.setMsg(dict.torrentManage2.deleted);
      opts.onDeleted();
    } catch (e) {
      opts.setMsg(
        e instanceof ApiError ? e.message : dict.common.networkError,
      );
    } finally {
      opts.setBusy(false);
    }
  }
  return { reseed, del };
}


/** 编辑提交载荷（0184 从 torrent-manage.tsx 拆出）：PUT /torrents/{id} body */
export function buildEditPayload(x: {
  fName: string;
  fSub: string;
  fDescr: string;
  fAnon: boolean;
  fCat: number;
  fImdb: string;
  fPoster: string;
  fMediainfo: string;
  fSec: Record<string, string>;
  /** B2：维度定义（编码 sections 时按 field_type 分派） */
  catKinds: SectionKindMeta[];
  fTags: number[];
  tagMine?: number[];
  dictRows: { id: number; kind: string }[];
  isStaff?: boolean;
  fPos: number;
  fPosUntil: string;
  fPick: number;
}) {
  return {
    name: x.fName.trim(),
        small_descr: x.fSub.trim(),
        descr: x.fDescr,
        anonymous: x.fAnon,
        category_id: x.fCat,
        imdb_id: x.fImdb.trim() || "",
        // 0173 补齐发布页字段：封面外链、
        // MediaInfo（None=不动 / Some("")=清除 / Some(text)=写入——始终提交）。
        // 媒介/学段/版本三个 legacy 列不再提交：维度归属统一走下面的 sections。
        poster: x.fPoster.trim(),
        mediainfo: x.fMediainfo.trim(),
        // 多维属性（B2 六类型）：按 field_type 编码 —— 枚举发整数（旧格式，
        // 后端零改动兼容）、多选发 {"dict_ids":[…]}、自由值发 {"text":…} 等；
        // 后端按**值的 JSON 类型**分派。
        sections: (() => {
          const out: Record<string, unknown> = {};
          for (const k of x.catKinds) {
            const v = x.fSec[k.kind] ?? "";
            if (!v) continue;
            const type = k.field_type ?? "select";
            if (type === "select") {
              const n = Number(v);
              if (n > 0) out[k.kind] = n;
            } else if (type === "multiselect") {
              const ids = v
                .split(",")
                .map((s) => Number(s.trim()))
                .filter((n) => n > 0);
              if (ids.length > 0) out[k.kind] = { dict_ids: ids };
            } else if (type === "number") {
              const n = Number(v);
              if (!Number.isNaN(n)) out[k.kind] = { number: n };
            } else if (type === "bool") {
              out[k.kind] = { bool: v === "true" };
            } else if (type === "date") {
              out[k.kind] = { date: v };
            } else {
              out[k.kind] = { text: v };
            }
          }
          return out;
        })(),
        // 标签整组提交（official 类保留不动：前端只编辑普通标签，
        // 后端 DELETE+apply 会把 official 一并清掉，所以这里带上原 official 集）
        tag_ids: [
          ...x.fTags,
          ...(x.tagMine ?? []).filter((id) =>
            x.dictRows.some((d) => d.id === id && d.kind === "official"),
          ),
        ],
        // 推荐位（0184）：staff 提交才带；后端 class>=90 强校验，
        // 非 staff 带值会 403 而不是静默吞掉
        ...(x.isStaff
          ? {
              pos_state: x.fPos,
              pos_state_until: x.fPosUntil
                ? new Date(x.fPosUntil).toISOString()
                : "",
              pick_type: x.fPick,
            }
          : {}),
      };
}

