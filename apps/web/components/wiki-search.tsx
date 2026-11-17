"use client";

/** Wiki 目录页检索框：即时过滤（客户端 DOM 过滤）。
 *
 *  卡片列表是 RSC 服务端直出的（SEO/无 JS 可见），这里不做数据流过滤——
 *  击键后按 [data-wiki-card] 的 data-title/data-slug 做包含匹配，
 *  命不中的卡片 display:none，命中空的分组卡（data-wiki-section）
 *  整卡隐藏。无框架状态、无重渲染，纯渐进增强：JS 失效时目录全量可见。 */
import { useId, useState } from "react";

import { WIKI_SEARCH_INPUT } from "@/lib/ui-classes";

export function WikiSearch({ placeholder }: { placeholder: string }) {
  const [q, setQ] = useState("");
  const id = useId();

  function filter(raw: string) {
    const needle = raw.trim().toLowerCase();
    setQ(raw);
    if (typeof document === "undefined") return;
    const cards = document.querySelectorAll<HTMLElement>("[data-wiki-card]");
    cards.forEach((el) => {
      const hit =
        !needle ||
        (el.dataset.title ?? "").toLowerCase().includes(needle) ||
        (el.dataset.slug ?? "").toLowerCase().includes(needle);
      el.style.display = hit ? "" : "none";
    });
    const sections =
      document.querySelectorAll<HTMLElement>("[data-wiki-section]");
    sections.forEach((el) => {
      const visible = [
        ...el.querySelectorAll<HTMLElement>("[data-wiki-card]"),
      ].some((c) => c.style.display !== "none");
      el.style.display = visible ? "" : "none";
    });
    const empty = document.getElementById("wiki-filter-empty");
    if (empty) {
      const any = [
        ...document.querySelectorAll<HTMLElement>("[data-wiki-card]"),
      ].some((c) => c.style.display !== "none");
      empty.style.display = any ? "none" : "";
    }
  }

  return (
    <div className="relative">
      <label htmlFor={id} className="sr-only">
        {placeholder}
      </label>
      <input
        id={id}
        type="search"
        value={q}
        placeholder={placeholder}
        onChange={(e) => filter(e.target.value)}
        className={WIKI_SEARCH_INPUT}
      />
      <span
        aria-hidden
        className={"pointer-events-none absolute right-4 top-1/2 " +
          "-translate-y-1/2 text-sub"}
      >
        🔍
      </span>
    </div>
  );
}
