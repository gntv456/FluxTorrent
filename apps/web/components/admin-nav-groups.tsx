"use client";

import { useState } from "react";
import type { PanelEntry } from "./admin-shell";

/** 折叠分组渲染（从 admin-shell 按域拆出，300 行门禁）。
 *  AdminNavGroups 左侧导航的分组体：三态折叠 + 待办徽章 + 条目按钮。
 *  二级子分类（NAV_SUBGROUPS）：大组按职能再分组，对齐样图 § 后台职能导航
 *  （内容/用户/运营/系统四个大组的工具数 15~24，平铺太长）。
 *  子分类**恒定展开**（样图默认态即展开）——折叠交给分组级，避免两级
 *  折叠状态互相打扰；子分类里未登记的 tab_key 走 loose 平铺回退，
 *  新工具进 DB 后不会因为没登记而消失。 */

/** [子类 id, [tab_key…]]；id 同时是 i18n `admin.navSubs` 的键。
 *  长数组按行拆写（行宽 ≤80 门禁），tab_key 顺序即导航展示顺序。 */
export const NAV_SUBGROUPS: Record<string, [string, string[]][]> = {
  content: [
    [
      "contentSite",
      [
        "torrents", "cats", "faq", "rules", "tagdict", "sections",
        "content", "menu", "ads", "userfields",
      ],
    ],
    ["contentForum", ["forums", "polls", "embedrules", "trumps"]],
    ["contentHonor", ["medals", "subawards"]],
    ["contentDoc", ["wiki", "pages", "terms"]],
  ],
  users: [
    [
      "usersAccount",
      [
        "users", "adduser", "resetpass", "deldisabled", "warned",
        "uploaders", "notconnect", "applications",
      ],
    ],
    ["usersSecurity", ["ipcheck", "hrpardon"]],
    [
      "usersReward",
      ["incrementbulk", "invites", "attendance", "roles", "gacha"],
    ],
    ["usersConfig", ["classes", "userlogs"]],
  ],
  ops: [
    ["opsSettings", ["settings"]],
    ["opsMessage", ["staffmess", "mail", "emailbans"]],
    ["opsRun", ["freeleech", "p2tools", "onboarding", "economy", "arcade"]],
    ["opsExam", ["exams", "tasks", "jixiao"]],
    ["opsBiz", ["donateorders", "fundings", "props"]],
  ],
  system: [
    ["sysStatus", ["stats", "dbstats", "syslog", "audit", "seedstats"]],
    ["sysClean", ["cleanup", "clearcache", "ops"]],
    ["sysNet", ["trackers", "agentrules", "agents", "locations"]],
    ["sysAccess", ["bans", "testip", "maxlogin", "emaildomains", "regquiz"]],
    ["sysPerm", ["perm", "plugins", "panelentries"]],
    ["sysCustom", ["homelayout", "viewlayout"]],
  ],
};

export interface NavGroup {
  key: string;
  label: string;
  items: PanelEntry[];
  /** 二级子分类（有定义的大组才非空） */
  subs?: { key: string; label: string; items: PanelEntry[] }[];
  /** 未登记进任何子分类的条目：平铺在分组末尾（新工具不消失） */
  loose?: PanelEntry[];
}

/** 按 NAV_SUBGROUPS 切分一个大组内的条目（顺序 = 登记顺序，空子类剔除） */
export function splitSubGroups(
  section: string,
  items: PanelEntry[],
  labelOf: (e: PanelEntry) => string,
): Pick<NavGroup, "subs" | "loose"> {
  const defs = NAV_SUBGROUPS[section];
  if (!defs) return {};
  const subs = defs
    .map(([key, keys]) => ({
      key,
      label: key,
      items: items.filter((e) => keys.includes(e.tab_key)),
    }))
    .filter((s) => s.items.length > 0);
  const covered = new Set(subs.flatMap((s) => s.items.map((e) => e.tab_key)));
  const loose = items.filter((e) => !covered.has(e.tab_key));
  return { subs, loose };
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
  subLabels,
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
  /** 二级子分类文案（i18n admin.navSubs，缺省回落 id） */
  subLabels: Record<string, string>;
}) {
  const [manualOpen, setManualOpen] = useState<Record<string, boolean>>({});
  /** 单行条目按钮；deep=子分类内的缩进档 */
  const itemRow = (e: PanelEntry, deep: boolean) => {
    const active = tool === e.tab_key;
    const n = badges[e.tab_key] ?? 0;
    return (
      <button
        key={e.tab_key}
        onClick={() => onTool(e.tab_key)}
        title={tipOf(e)}
        className={`flex min-h-[34px] items-center justify-between
          gap-2 rounded-[var(--r-md)] text-left text-[13px] transition ${
            deep ? "pl-7 pr-2" : "px-2"
          } ${
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
  };
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
                {g.subs?.map((s) => (
                  <div key={s.key}>
                    <div
                      className="mx-1 mb-0.5 mt-1.5 flex items-center gap-1.5
                        rounded-[5px] bg-[var(--surface-raised)] px-2 py-1
                        text-[10.5px] font-extrabold tracking-wider
                        text-[var(--text-brand)]"
                    >
                      <span
                        className="inline-block h-[11px] w-[3px]
                          rounded-[2px] bg-[var(--sky)]"
                      />
                      {subLabels[s.key] ?? s.label}
                    </div>
                    <div className="flex flex-col gap-0.5">
                      {s.items.map((e) => itemRow(e, true))}
                    </div>
                  </div>
                ))}
                {g.loose?.map((e) => itemRow(e, false))}
                {(!g.subs || g.subs.length === 0) &&
                  g.items.map((e) => itemRow(e, false))}
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
