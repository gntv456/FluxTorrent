"use client";

import { useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface Hit {
  topic_id: number;
  title: string;
  forum_id: number;
  forum_name: string;
  author: string | null;
  created_at: string;
  replies: number;
  locked: boolean;
}

/** 论坛标题搜索（NP 顶栏搜帖口径）：GET /forums/search，结果内联展示 */
export function ForumSearch({ placeholder, button }: { placeholder: string; button: string }) {
  const { dict, locale } = useI18n();
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<Hit[] | null>(null);
  const [busy, setBusy] = useState(false);

  async function go(e: React.FormEvent) {
    e.preventDefault();
    if (q.trim().length < 2) return;
    setBusy(true);
    try {
      setHits(await api.get<Hit[]>(`/api/v1/forums/search?q=${encodeURIComponent(q.trim())}`));
    } catch {
      setHits([]);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <form onSubmit={go} className="flex gap-2">
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={placeholder}
          className="min-h-[36px] flex-1 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-3 text-sm"
        />
        <button
          type="submit"
          disabled={busy}
          className="min-h-[36px] rounded-full bg-sky-deep px-4 text-sm text-white disabled:opacity-50"
        >
          {button}
        </button>
      </form>
      {hits !== null && (
        <div className="baozi-wide-table-scroll rounded-[var(--r-md)] border border-line">
          <table className="nexus-table">
            <tbody>
              {hits.map((h) => (
                <tr key={h.topic_id}>
                  <td className="max-w-[360px] truncate">
                    <Link href={`/forums/topic/${h.topic_id}`} className="text-sky-deep hover:underline">
                      {h.locked ? "🔒 " : ""}
                      {h.title}
                    </Link>
                    <p className="text-xs text-sub">
                      {h.author ?? dict.torrent.anonymous} ·{" "}
                      {dict.forums.replies.replace("{n}", String(h.replies))} ·{" "}
                      {new Date(h.created_at).toLocaleDateString(dateLocale(locale))}
                    </p>
                  </td>
                  <td className="shrink-0 text-xs text-sub">{h.forum_name}</td>
                </tr>
              ))}
              {hits.length === 0 && (
                <tr>
                  <td className="py-4 text-center text-sm text-sub">{dict.forums2.searchEmpty}</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
