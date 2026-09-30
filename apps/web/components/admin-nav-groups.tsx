"use client";

import { useState } from "react";
import type { PanelEntry } from "./admin-shell";

/** 折叠分组渲染（从 admin-shell 按域拆出，300 行门禁）。
 *  AdminNavGroups 左侧导航的分组体：三态折叠 + 待办徽章 + 条目按钮。 */

export interface NavGroup {
  key: string;
  label: string;
  items: PanelEntry[];
}

/**
 * 三态折叠：manualOpen[key] 未设置 = 自动（只展开当前工具/有待办徽章的
 * 分组，徽章收起后分组头仍显示聚合数）；显式 true/false 为手动覆盖。
 * 搜索中全部展开（结果要跨分组可见）。⚠️ 展开判定不能写成
 * `!manualOpen[key]`——初始 {} 会让六个组全展开，折叠默认形同虚设。
 */
export function AdminNavGroups({
  grouped,
  badges,
  tool,
  onTool,
  kwEmpty,
  labelOf,
  tipOf,
  navEmpty,
}: {
  grouped: NavGroup[];
  badges: Record<string, number>;
  /** 当前工具 tab_key（决定自动展开哪个分组） */
  tool: string;
  onTool: (t: string) => void;
  /** 搜索关键词为空（true=折叠态生效；false=全展开） */
  kwEmpty: boolean;
  labelOf: (e: PanelEntry) => string;
  tipOf: (e: PanelEntry) => string;
  /** 搜索无命中提示文案 */
  navEmpty: string;
}) {
  const [manualOpen, setManualOpen] = useState<Record<string, boolean>>({});
  return (
    <div className="flex flex-col gap-3">
      {grouped.map((g) => {
        const groupBadges = g.items.reduce(
          (s, e) => s + (badges[e.tab_key] ?? 0),
          0,
        );
        const expanded =
          !kwEmpty ||
          (manualOpen[g.key] ??
            (g.items.some((e) => e.tab_key === tool) || groupBadges > 0));
        return (
          <div key={g.key}>
            <button
              type="button"
              onClick={() =>
                setManualOpen((f) => ({ ...f, [g.key]: !expanded }))
              }
              aria-expanded={expanded}
              className="mb-1 flex w-full items-center justify-between px-2
                text-xs font-bold text-sub hover:text-ink"
            >
              <span>{g.label}</span>
              <span className="flex items-center gap-1">
                {!expanded && groupBadges > 0 && (
                  <span
                    className={`shrink-0 rounded-full bg-coral/20 px-1.5
                      text-[11px] text-danger`}
                  >
                    {groupBadges}
                  </span>
                )}
                <span className="text-[10px]">{expanded ? "▾" : "▸"}</span>
              </span>
            </button>
            {expanded && (
              <div className="flex flex-col gap-0.5">
                {g.items.map((e) => {
                  const active = tool === e.tab_key;
                  const n = badges[e.tab_key] ?? 0;
                  return (
                    <button
                      key={e.tab_key}
                      onClick={() => onTool(e.tab_key)}
                      title={tipOf(e)}
                      className={`flex min-h-[34px] items-center justify-between
                        gap-2 rounded-[var(--r-md)] px-2 text-left text-[13px]
                        transition ${
                          active
                            ? "bg-sky font-bold text-white"
                            : "text-sub hover:bg-[var(--surface-raised)]"
                        }`}
                    >
                      <span className="truncate">{labelOf(e)}</span>
                      {n > 0 && (
                        <span
                          className={`shrink-0 rounded-full px-1.5
                            text-[11px] ${
                              active
                                ? "bg-white/25 text-white"
                                : "bg-coral/20 text-danger"
                            }`}
                        >
                          {n}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
      {grouped.length === 0 && (
        <p className="px-2 text-xs text-sub">{navEmpty}</p>
      )}
    </div>
  );
}
