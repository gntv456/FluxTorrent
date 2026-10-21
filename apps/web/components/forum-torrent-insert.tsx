"use client";

import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/**
 * 论坛「引用种子」插入行（与 VideoInsertRow 同族、同 onInsert 契约）：
 * 输入 ≥2 字符防抖 300ms 调 /torrents/suggest，下拉键盘导航（上下/回车/
 * Esc/点外关），选中回调 onInsert("!torrent(id)")——由宿主 composer 追加
 * 到正文（appendVideoBlock 通用空行分隔追加）。候选行复用 tsb-suggest__*
 * 样式类（下拉部分另加 fti 前缀定位在自身 wrapper 内）。
 */

interface SuggestItem {
  id: number;
  name: string;
  small_descr?: string | null;
  seeders: number;
}

export function TorrentInsertRow({
  onInsert,
}: {
  onInsert: (line: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.forums;
  const [q, setQ] = useState("");
  const [items, setItems] = useState<SuggestItem[]>([]);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const [busy, setBusy] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const query = q.trim();
    if (query.length < 2) {
      setItems([]);
      setOpen(false);
      return;
    }
    let aborted = false;
    const timer = setTimeout(async () => {
      setBusy(true);
      try {
        const r = await api.get<{ items?: SuggestItem[] }>(
          `/api/v1/torrents/suggest?q=${encodeURIComponent(query)}`,
        );
        if (aborted) return;
        setItems(r?.items ?? []);
        setOpen(true);
        setActive(-1);
      } catch {
        if (!aborted) setOpen(false);
      } finally {
        if (!aborted) setBusy(false);
      }
    }, 300);
    return () => {
      aborted = true;
      clearTimeout(timer);
      setBusy(false);
    };
  }, [q]);

  useEffect(() => {
    const onDocClick = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("click", onDocClick);
    return () => document.removeEventListener("click", onDocClick);
  }, []);

  function pick(it: SuggestItem) {
    onInsert(`!torrent(${it.id})`);
    setQ("");
    setItems([]);
    setOpen(false);
    setActive(-1);
  }

  function onKey(e: React.KeyboardEvent) {
    if (!open || items.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => (a + 1) % items.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => (a <= 0 ? items.length - 1 : a - 1));
    } else if (e.key === "Escape") {
      setOpen(false);
    } else if (e.key === "Enter" && active >= 0) {
      e.preventDefault();
      const it = items[active];
      if (it) pick(it);
    }
  }

  return (
    <div ref={wrapRef} className="flex flex-col gap-1">
      <span className="text-sm text-sub">{t.torrentInsertTitle}</span>
      <div className="relative flex items-center gap-1.5">
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={onKey}
          placeholder={t.torrentSearchPh}
          maxLength={80}
          role="combobox"
          aria-expanded={open}
          aria-controls="fti-list"
          className={
            "w-full min-h-[38px] rounded-[var(--r-sm)] border " +
            "border-line bg-cloud px-3 text-sm outline-none " +
            "focus:border-sky"
          }
        />
        {busy && (
          <span className="absolute right-3 text-xs text-sub" aria-hidden>
            …
          </span>
        )}
        {open && items.length > 0 && (
          <div
            id="fti-list"
            role="listbox"
            className="tsb-suggest fti-pop"
            aria-label={t.torrentInsertTitle}
          >
            {items.map((it, i) => (
              <button
                type="button"
                key={it.id}
                role="option"
                aria-selected={i === active}
                className={`tsb-suggest__item${
                  i === active ? " is-active" : ""
                }`}
                onMouseEnter={() => setActive(i)}
                onClick={() => pick(it)}
              >
                <span className="tsb-suggest__name">{it.name}</span>
                <span className="tsb-suggest__meta">
                  {it.seeders > 0 ? `${it.seeders} ↑` : "—"}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
