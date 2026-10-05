"use client";

import { useEffect, useRef, useState } from "react";

/**
 * 搜索输入联想（0283 P0-2）：输入 ≥2 字符防抖 300ms 调 /torrents/suggest，
 * 下拉展示候选标题（回车仍走表单提交的常规搜索；点候选直接跳详情）。
 * 服务端组件包裹层的 input 保持受控于 form——本组件只做 overlay，不改表单语义。
 */

interface SuggestItem {
  id: number;
  name: string;
  small_descr?: string | null;
  seeders: number;
}

export function SearchSuggest({
  inputId = "searchinput",
}: {
  inputId?: string;
}) {
  const [items, setItems] = useState<SuggestItem[]>([]);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const input = document.getElementById(
      inputId,
    ) as HTMLInputElement | null;
    if (!input) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let aborted = false;

    const onInput = () => {
      const q = input.value.trim();
      if (timer) clearTimeout(timer);
      if (q.length < 2) {
        setItems([]);
        setOpen(false);
        return;
      }
      timer = setTimeout(async () => {
        try {
          const res = await fetch(
            `/api/v1/torrents/suggest?q=${encodeURIComponent(q)}`,
          );
          if (!res.ok) return;
          const json = (await res.json()) as {
            data?: { items?: SuggestItem[] };
          };
          if (aborted) return;
          setItems(json.data?.items ?? []);
          setOpen(true);
          setActive(-1);
        } catch {
          /* 联想失败静默：不打扰手动搜索 */
        }
      }, 300);
    };

    const onKey = (e: KeyboardEvent) => {
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
        if (it) window.location.href = `/torrent/${it.id}`;
      }
    };

    const onDocClick = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };

    input.addEventListener("input", onInput);
    input.addEventListener("keydown", onKey);
    document.addEventListener("click", onDocClick);
    return () => {
      aborted = true;
      if (timer) clearTimeout(timer);
      input.removeEventListener("input", onInput);
      input.removeEventListener("keydown", onKey);
      document.removeEventListener("click", onDocClick);
    };
  }, [inputId, open, items.length, active]);

  if (!open || items.length === 0) return null;
  return (
    <div
      ref={wrapRef}
      className="tsb-suggest"
      role="listbox"
      aria-label="search suggestions"
    >
      {items.map((it, i) => (
        <a
          key={it.id}
          href={`/torrent/${it.id}`}
          role="option"
          aria-selected={i === active}
          className={`tsb-suggest__item${i === active ? " is-active" : ""}`}
          onMouseEnter={() => setActive(i)}
        >
          <span className="tsb-suggest__name">{it.name}</span>
          <span className="tsb-suggest__meta">
            {it.seeders > 0 ? `${it.seeders} ↑` : "—"}
          </span>
        </a>
      ))}
    </div>
  );
}
