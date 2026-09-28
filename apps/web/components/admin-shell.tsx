"use client";

import { useEffect, useRef, useState } from "react";
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

/**
 * 管理后台外壳：左侧常驻职能导航 + 右侧内容区 + 顶部全局搜索。
 *
 * - 分组维度是职能（section），权限只作过滤（min_class），无权条目后端已剔除
 * - 待办数字常驻导航，切到任何页面都可见
 * - Ctrl/⌘ + K 聚焦搜索，实时过滤导航
 * - **导航是管理端唯一入口**：面板内部曾另有一行 35 个药丸按钮，二者同屏重复
 *   （药丸行占 184px），已删除。
 *
 * 导航文案：`name` / `info` 存在 DB（`staff_panel_entries`），DB 值即
 * **zh-CN 的权威文案**；`en` / `zh-TW` 由 i18n 的 `adminNav[tab_key]` 覆盖，
 * 取不到时回落 DB 值 —— 这样新增工具只在 DB 插一行也能立刻可用（显示中文），
 * 不会渲染成空白。见 i18n/*.ts 的 adminNav 段。
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
  const nav = dict.adminNav as unknown as Record<
    string,
    { label: string; tip: string } | undefined
  >;
  // 徽章优先显示真实等级名（等级体系已扩到 12 级用户层 + 管理职级）
  const classLabel =
    classId !== undefined
      ? dict.admin.classList.find(([id]) => id === classId)?.[1]
      : undefined;
  const ROLE_LABEL: Record<string, string> = {
    sysop: a.roleSysop,
    administrator: a.roleAdmin,
    moderator: a.roleModerator,
  };
  const [q, setQ] = useState("");
  const [navOpen, setNavOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  /** 分组标题：i18n 缺失时回落 section key（英文），不留中文兜底 */
  const LABEL: Record<string, string> = {
    dashboard: a.sectionDashboard ?? "dashboard",
    moderation: a.sectionModeration ?? "moderation",
    users: a.sectionUsers ?? "users",
    content: a.sectionContent ?? "content",
    ops: a.sectionOps ?? "ops",
    system: a.sectionSystem ?? "system",
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

  /** 导航文案：i18n 优先，回落 DB 文案（DB 即 zh-CN 权威值） */
  const labelOf = (e: PanelEntry) => nav[e.tab_key]?.label ?? e.name;
  const tipOf = (e: PanelEntry) => nav[e.tab_key]?.tip ?? e.info;

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
  // 视图布局（E6）入口：同 homelayout 恒定注入（保存端点 SETTINGS_MANAGE）
  const viewLayoutEntry: PanelEntry = {
    section: "system",
    name: dict.viewLayout.title,
    url: "/admin?tool=viewlayout",
    info: dict.viewLayout.hint,
    tab_key: "viewlayout",
    min_class: 99,
  };
  const withOps = entries.some((e) => e.tab_key === "ops")
    ? entries
    : [...entries, opsEntry];
  const withHome = withOps.some((e) => e.tab_key === "homelayout")
    ? withOps
    : [...withOps, homeLayoutEntry];
  const allEntries = withHome.some((e) => e.tab_key === "viewlayout")
    ? withHome
    : [...withHome, viewLayoutEntry];

  // 59 条过滤，成本可忽略；不用 useMemo —— 它的依赖（labelOf/tipOf/LABEL
  // 都随渲染重建）只会带来 useEffect 依赖陈旧的风险，换不来实际收益。
  const kw = q.trim().toLowerCase();
  const grouped = SECTION_ORDER.map((key) => ({
    key,
    label: LABEL[key] ?? key,
    items: allEntries.filter(
      (e) =>
        e.section === key &&
        (!kw ||
          `${labelOf(e)}${tipOf(e)}${e.tab_key}`.toLowerCase().includes(kw)),
    ),
  })).filter((g) => g.items.length > 0);

  const navBody = (
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
                  title={tipOf(e)}
                  className={`flex min-h-[34px] items-center justify-between gap-2 rounded-[var(--r-md)] px-2 text-left text-[13px] transition ${
                    active
                      ? "bg-sky font-bold text-white"
                      : "text-sub hover:bg-[var(--surface-raised)]"
                  }`}
                >
                  <span className="truncate">{labelOf(e)}</span>
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
        <p className="px-2 text-xs text-sub">{a.navEmpty}</p>
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
            placeholder={a.navSearchPlaceholder}
            aria-label={a.navSearchLabel}
            className="min-h-[38px] w-full rounded-[var(--r-md)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          />
        </div>
        <span
          className="shrink-0 rounded-full bg-sky-soft px-3 py-1 text-xs font-bold text-sky"
          title={
            ROLE_LABEL[role]
              ? a.roleTitle.replace("{role}", ROLE_LABEL[role]!)
              : undefined
          }
        >
          {classLabel ? `${classId} ${classLabel}` : (ROLE_LABEL[role] ?? role)}
        </span>
        <button
          onClick={() => setNavOpen(!navOpen)}
          aria-expanded={navOpen}
          className="min-h-[38px] rounded-[var(--r-md)] border border-line px-3 text-sm font-bold lg:hidden"
        >
          {a.navToggle}
        </button>
      </div>

      <div className="flex flex-col gap-4 lg:flex-row">
        <nav
          aria-label={a.navAriaLabel}
          className="hidden w-[200px] shrink-0 lg:block lg:sticky lg:top-4 lg:max-h-[calc(100vh-8rem)] lg:self-start lg:overflow-y-auto lg:pr-1"
        >
          {navBody}
        </nav>
        {/* M2：移动端管理导航从「内联展开顶出内容」改左侧滑入抽屉
            （复用 navdrawer 骨架；条目是 onTool button 而非路由 link） */}
        {navOpen && (
          <div className="navdrawer-root lg:hidden" role="dialog" aria-modal="true">
            <div className="navdrawer-mask" onClick={() => setNavOpen(false)} />
            <nav aria-label={a.navAriaLabel} className="navdrawer admin-navdrawer">
              <div className="navdrawer__head">
                <p className="navdrawer__name">{a.navAriaLabel}</p>
                <button
                  type="button"
                  className="navdrawer__close"
                  aria-label={a.navToggle}
                  onClick={() => setNavOpen(false)}
                >
                  ✕
                </button>
              </div>
              <div className="navdrawer__body">{navBody}</div>
            </nav>
          </div>
        )}
        {/* M1 裸表兜底：管理面板内容区的裸 <table> 在 <lg 直接撑破容器。
            CSS 侧（pages.css .admin-panel-content table）统一转块级横滚，
            组件文件零改动即可救活 39+ 张表；显式包裹的表格不受影响。 */}
        <div className="admin-panel-content min-w-0 flex-1">{children}</div>
      </div>
    </div>
  );
}
