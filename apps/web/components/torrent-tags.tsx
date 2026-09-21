"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

interface TagDictRow {
  id: number;
  name: string;
  kind: string;
}

/** 兼容 API 历史形态：dict 行可能序列化为 [id, name, kind] 元组 */
function normTagRow(r: TagDictRow | [number, string, string]): TagDictRow {
  return Array.isArray(r) ? { id: r[0], name: r[1], kind: r[2] } : r;
}

/** 种子标签（T-04）：作者/staff 打标，官种/官方标签仅 staff；点击切换 */
export function TorrentTags({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const t = dict.torrTags2 ?? {
    title: "标签",
    needStaff: "官方标签仅管理组可打",
  };
  const [dictRows, setDictRows] = useState<TagDictRow[]>([]);
  const [mine, setMine] = useState<number[]>([]);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const r = await api.get<{
        dict: (TagDictRow | [number, string, string])[];
        mine: number[];
      }>(`/api/v1/torrents/${torrentId}/tags`);
      setDictRows(r.dict.map(normTagRow));
      setMine(r.mine);
    } catch {
      setDictRows([]);
    }
  }, [torrentId]);

  useEffect(() => {
    load();
  }, [load]);

  async function toggle(tagId: number, on: boolean) {
    setMsg(null);
    try {
      await api.put(`/api/v1/torrents/${torrentId}/tags`, {
        tag_id: tagId,
        on: !on,
      });
      setMine((m) => (on ? m.filter((x) => x !== tagId) : [...m, tagId]));
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    }
  }

  if (dictRows.length === 0) return null;

  return (
    <div className="td-tagcloud">
      {dictRows.map((d, i) => {
        const on = mine.includes(d.id);
        const official = d.kind === "official";
        // 列表口径的彩色轮换（官方类固定靛蓝，普通标签按位轮换品牌色）
        const palette = [
          "torrents-tag--sky",
          "torrents-tag--mint",
          "torrents-tag--sun",
          "torrents-tag--candy",
        ];
        const colorCls = official
          ? "torrents-tag--official"
          : palette[i % palette.length];
        return (
          <button
            key={d.id}
            type="button"
            onClick={() => toggle(d.id, on)}
            title={official ? t.needStaff : d.name}
            className={`td-tag ${colorCls} ${on ? "" : "td-tag--off"}`}
          >
            {d.name}
          </button>
        );
      })}
      {msg && (
        <span className="text-[11px] text-sub" role="status">
          {msg}
        </span>
      )}
    </div>
  );
}
