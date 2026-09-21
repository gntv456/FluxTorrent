"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "@/i18n/client";

/** 管理面板条目（后端 /admin/staffpanel 返回，已按 min_class 过滤） */
export interface PanelEntry {
  section: string;
  name: string;
  url: string;
  info: string;
  tab_key: string;
  min_class: number;
}

/** 职能分组顺序（不是权限等级顺序） */
export const SECTION_ORDER = [
  "dashboard",
  "moderation",
  "users",
  "content",
  "ops",
  "system",
] as const;

const ROLE_LABEL: Record<string, string> = {
  sysop: "SysOp",
  administrator: "管理员",
  moderator: "版主",
};

/**
 * 管理后台外壳：左侧常驻职能导航 + 右侧内容区 + 顶部全局搜索。
 *
 * 替代原先的「四组药丸带 + 32 个平铺按钮」：
 * - 分组维度是职能（section），权限只作过滤（min_class），无权条目后端已剔除
 * - 待办数字常驻导航，切到任何页面都可见
 * - Ctrl/⌘ + K 聚焦搜索，实时过滤导航
 */
export function AdminShell({
  entries,
  tool,
  onTool,
  badges,
  role,
  classId,
  children,
}: {
  entries: PanelEntry[];
  tool: string;
  onTool: (t: string) => void;
  badges: Record<string, number>;
  role: string;
  classId?: number;
  children: React.ReactNode;
}) {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const ops = dict.adminops;
  // 徽章优先显示真实等级名（等级体系已扩到 12 级用户层 + 管理职级）
  const classLabel =
    classId !== undefined
      ? dict.admin.classList.find(([id]) => id === classId)?.[1]
      : undefined;
  const [q, setQ] = useState("");
  const [navOpen, setNavOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const LABEL: Record<string, string> = {
    dashboard: a.sectionDashboard ?? "工作台",
    moderation: a.sectionModeration ?? "审核队列",
    users: a.sectionUsers ?? "用户",
    content: a.sectionContent ?? "内容",
    ops: a.sectionOps ?? "运营",
    system: a.sectionSystem ?? "系统",
  };

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // 运维三件套（0078）入口：面板条目由 DB 种子驱动，可能未含该项——
  // 面板里恒定补一条「运维」（后端 /admin/version 等按权限校验，无权时面板内报错）
  const opsEntry: PanelEntry = {
    section: "system",
    name: ops.entry,
    url: "/admin?tool=ops",
    info: `${ops.versionTitle} / ${ops.backupsTitle} / ${ops.jobsTitle}`,
    tab_key: "ops",
    min_class: 90,
  };
  // 首页排版（0089）入口：同样恒定注入（后端保存端点要求 SETTINGS_MANAGE=99，
  // 无权时面板内保存报错；列表仅 sysop 可见，min_class=99）
  const homeLayoutEntry: PanelEntry = {
    section: "system",
    name: dict.homeLayout.title,
    url: "/admin?tool=homelayout",
    info: dict.homeLayout.hint,
    tab_key: "homelayout",
    min_class: 99,
  };
  const allEntries = entries.some((e) => e.tab_key === "ops")
    ? entries
    : [...entries, opsEntry];
  const allEntries2 = allEntries.some((e) => e.tab_key === "homelayout")
    ? allEntries
    : [...allEntries, homeLayoutEntry];

  const grouped = useMemo(() => {
    const kw = q.trim().toLowerCase();
    return SECTION_ORDER.map((key) => ({
      key,
      label: LABEL[key] ?? key,
      items: allEntries2.filter(
        (e) =>
          e.section === key &&
          (!kw || `${e.name}${e.info}${e.tab_key}`.toLowerCase().includes(kw)),
      ),
    })).filter((g) => g.items.length > 0);
  }, [allEntries, q, a]);

  const nav = (
    <div className="flex flex-col gap-3">
      {grouped.map((g) => (
        <div key={g.key}>
          <p className="mb-1 px-2 text-xs font-bold text-sub">{g.label}</p>
          <div className="flex flex-col gap-0.5">
            {g.items.map((e) => {
              const active = tool === e.tab_key;
              const n = badges[e.tab_key] ?? 0;
              return (
                <button
                  key={e.tab_key}
                  onClick={() => {
                    onTool(e.tab_key);
                    setNavOpen(false);
                  }}
                  title={e.info}
                  className={`flex min-h-[34px] items-center justify-between gap-2 rounded-[var(--r-md)] px-2 text-left text-[13px] transition ${
                    active
                      ? "bg-sky font-bold text-white"
                      : "text-sub hover:bg-[var(--surface-raised)]"
                  }`}
                >
                  <span className="truncate">{e.name}</span>
                  {n > 0 && (
                    <span
                      className={`shrink-0 rounded-full px-1.5 text-[11px] ${
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
        </div>
      ))}
      {grouped.length === 0 && (
        <p className="px-2 text-xs text-sub">没有匹配的工具</p>
      )}
    </div>
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">{a.panelTitle}</h1>
        <div className="min-w-[180px] flex-1">
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="搜索工具…（Ctrl/⌘ + K）"
            aria-label="搜索管理工具"
            className="min-h-[38px] w-full rounded-[var(--r-md)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          />
        </div>
        <span
          className="shrink-0 rounded-full bg-sky-soft px-3 py-1 text-xs font-bold text-sky"
          title={ROLE_LABEL[role] ? `系统角色：${ROLE_LABEL[role]}` : undefined}
        >
          {classLabel ? `${classId} ${classLabel}` : (ROLE_LABEL[role] ?? role)}
        </span>
        <button
          onClick={() => setNavOpen(!navOpen)}
          aria-expanded={navOpen}
          className="min-h-[38px] rounded-[var(--r-md)] border border-line px-3 text-sm font-bold lg:hidden"
        >
          导航
        </button>
      </div>

      <div className="flex flex-col gap-4 lg:flex-row">
        <nav
          aria-label="管理功能导航"
          className="hidden w-[200px] shrink-0 lg:block lg:sticky lg:top-4 lg:max-h-[calc(100vh-8rem)] lg:self-start lg:overflow-y-auto lg:pr-1"
        >
          {nav}
        </nav>
        {navOpen && (
          <nav
            aria-label="管理功能导航"
            className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-2 lg:hidden"
          >
            {nav}
          </nav>
        )}
        <div className="min-w-0 flex-1">{children}</div>
      </div>
    </div>
  );
}
