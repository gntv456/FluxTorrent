"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { SubtitleItem } from "@/lib/data";

/** 字幕区（包子站 subtitles.php 同款）：列表 + 登记上传（+5 火花） */
export function SubtitleBoard({ empty }: { empty: string }) {
  const { dict } = useI18n();
  const [items, setItems] = useState<SubtitleItem[] | null>(null);
  const [title, setTitle] = useState("");
  const [lang, setLang] = useState("chs");
  const [torrentId, setTorrentId] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    try {
      setItems(await api.get<SubtitleItem[]>("/api/v1/subtitles"));
    } catch {
      setItems([]);
    }
  }
  useEffect(() => {
    refresh();
  }, []);

  async function upload() {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/subtitles", {
        torrent_id: torrentId ? Number(torrentId) : 0,
        title,
        lang,
      });
      setTitle("");
      setMsg(dict.subtitles.reward);
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  if (items === null) return null;
  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
        <h2 className="font-bold">{dict.subtitles.uploadTitle}</h2>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={dict.subtitles.titlePlaceholder}
            className="min-h-[44px] flex-1 rounded-[var(--r-sm)] border border-line bg-white px-3"
          />
          <select
            value={lang}
            onChange={(e) => setLang(e.target.value)}
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-white px-3"
          >
            <option value="chs">简体</option>
            <option value="cht">繁體</option>
            <option value="eng">English</option>
            <option value="jpn">日本語</option>
          </select>
          <input
            value={torrentId}
            onChange={(e) => setTorrentId(e.target.value.replace(/\D/g, ""))}
            placeholder="ID"
            className="min-h-[44px] w-24 rounded-[var(--r-sm)] border border-line bg-white px-3"
          />
          <button
            type="button"
            disabled={busy || !title.trim()}
            onClick={upload}
            className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
          >
            {dict.subtitles.uploadTitle}
          </button>
        </div>
        {msg && (
          <p role="alert" className="mt-2 text-sm text-sky-deep">
            {msg}
          </p>
        )}
      </div>

      {items.length === 0 ? (
        <p className="py-8 text-center text-sub">{empty}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {items.map((s) => (
            <li
              key={s.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
            >
              <div className="min-w-0">
                <p className="truncate font-bold">{s.title}</p>
                <p className="text-xs text-sub">
                  {s.username ?? "—"}
                  {s.torrent_id ? ` · #${s.torrent_id}` : ""}
                </p>
              </div>
              <span className="sticker bg-cloud text-sub num">⬇ {s.downloads}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
