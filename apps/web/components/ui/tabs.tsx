"use client";
import { useRef, type ReactNode } from "react";

/**
 * 标签组：复用 theme-tide.css 的 .tide-tabs / .tide-tab 原语
 * （medal-hall、shop-catalog 已在用同一套类），补上它们没有做的
 * roving tabindex 与方向键，供后续页面直接复用而不必各写一份。
 *
 * 约定：idPrefix 决定 DOM id（tab-<前缀>-<key> / panel-<前缀>-<key>），
 * 同页多组标签靠它互不撞号；面板用 <TabPanel> 包，保持
 * aria-controls ↔ aria-labelledby 双向指认。
 *
 * 两档切换（与 Pager 同形）：
 *   回调档 —— 传 onChange，标签是 <button>，方向键 + roving tabindex；
 *   链接档 —— 传 hrefs（与 items 同序），标签是 <a>，SSR 页面切换。
 *     RSC 传不了函数，所以链接档不拦方向键：链接本身就该被 Tab 键逐个
 *     停留，做 roving 反而把可聚焦项藏掉；也不指 aria-controls——SSR
 *     一次只渲染当前那一个面板，指过去就是悬空 id。
 */
export interface TabItem {
  key: string;
  label: string;
}

export function Tabs({
  items,
  value,
  onChange,
  hrefs,
  label,
  idPrefix,
}: {
  items: TabItem[];
  value: string;
  onChange?: (key: string) => void;
  hrefs?: string[];
  label: string;
  idPrefix: string;
}) {
  const linkMode = !!hrefs && hrefs.length === items.length;
  const go = (delta: number) => {
    const i = items.findIndex((t) => t.key === value);
    const next = items[(i + delta + items.length) % items.length];
    if (next) onChange?.(next.key);
  };
  return (
    <div
      className="tide-tabs"
      role="tablist"
      aria-label={label}
      onKeyDown={
        linkMode
          ? undefined
          : (e) => {
              if (e.key === "ArrowRight") {
                e.preventDefault();
                go(1);
              } else if (e.key === "ArrowLeft") {
                e.preventDefault();
                go(-1);
              } else if (e.key === "Home") {
                e.preventDefault();
                onChange?.(items[0]?.key ?? value);
              } else if (e.key === "End") {
                e.preventDefault();
                onChange?.(items[items.length - 1]?.key ?? value);
              }
            }
      }
    >
      {items.map((it, i) =>
        linkMode ? (
          <a
            key={it.key}
            href={hrefs?.[i]}
            role="tab"
            id={`tab-${idPrefix}-${it.key}`}
            className="tide-tab"
            aria-selected={value === it.key}
            aria-current={value === it.key ? "page" : undefined}
          >
            {it.label}
          </a>
        ) : (
          <button
            key={it.key}
            type="button"
            role="tab"
            id={`tab-${idPrefix}-${it.key}`}
            className="tide-tab"
            aria-selected={value === it.key}
            aria-controls={`panel-${idPrefix}-${it.key}`}
            tabIndex={value === it.key ? 0 : -1}
            onClick={() => onChange?.(it.key)}
          >
            {it.label}
          </button>
        ),
      )}
    </div>
  );
}

export function TabPanel({
  idPrefix,
  tabKey,
  children,
}: {
  idPrefix: string;
  tabKey: string;
  children: ReactNode;
}) {
  return (
    <div
      role="tabpanel"
      id={`panel-${idPrefix}-${tabKey}`}
      aria-labelledby={`tab-${idPrefix}-${tabKey}`}
      tabIndex={0}
    >
      {children}
    </div>
  );
}
