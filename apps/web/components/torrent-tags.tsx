"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

interface TagDictRow {
  id: number;
  name: string;
  kind: string;
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
      const r = await api.get<{ dict: TagDictRow[]; mine: number[] }>(
        `/api/v1/torrents/${torrentId}/tags`,
      );
      setDictRows(r.dict);
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
      await api.put(`/api/v1/torrents/${torrentId}/tags`, { tag_id: tagId, on: !on });
      setMine((m) => (on ? m.filter((x) => x !== tagId) : [...m, tagId]));
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    }
  }

  if (dictRows.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-1">
      {dictRows.map((d) => {
        const on = mine.includes(d.id);
        return (
          <button
            key={d.id}
            type="button"
            onClick={() => toggle(d.id, on)}
            title={d.kind === "official" ? t.needStaff : d.name}
            className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
              on
                ? d.kind === "official"
                  ? "bg-indigo text-white"
                  : "bg-sky text-white"
                : "border border-line bg-[var(--surface-card)] text-sub"
            }`}
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
