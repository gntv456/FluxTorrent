"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { usePathname } from "next/navigation";
import { useI18n } from "@/i18n/client";
import { GlobalSearch } from "@/components/global-search";
import { ThemeToggle } from "@/components/theme-toggle";
import { useFocusTrap } from "@/lib/use-focus-trap";
import type { NavItem, NavGroup } from "@/lib/nav-menu";
import {
  buildNavTree,
  claimedPaths,
  isActive,
  leavesOf,
  pathToActive,
  subtreeActive,
  type NavNode,
} from "@/lib/nav-tree";

/** 移动导航抽屉（M2）：与桌面 MainMenu 同源（lib/nav-tree 的同一棵树），
 *  左侧滑入 280ms var(--ease-out)。<lg 承运全部层级：一级 → 分组 →
 *  资源库分类 / 教育年级 / 科目，逐级折叠，链接一个不缺。
 *  用户摘要行由调用方传入（RSC 侧取 /me/overview 摘要或回落游客文案）。
 *  关闭：遮罩点击 / ESC / 右缘 48px 内 pointerdown（手势关闭）。
 *  打开期间锁 body 滚动；路由切换自动关闭。 */

type NodeProps = {
  node: NavNode;
  depth: number;
  openIds: Set<string>;
  active: (href: string) => boolean;
  onToggle: (id: string) => void;
  onClose: () => void;
};

/** 递归节点：叶子=链接；带子级=可折叠分组（指针/键盘均走同一 aria 语义）。 */
function DrawerNode({
  node,
  depth,
  openIds,
  active,
  onToggle,
  onClose,
}: NodeProps) {
  const hasKids = node.children.length > 0;
  const isOpen = openIds.has(node.id);
  const lit =
    active(node.href) || (hasKids && subtreeActive(node.children, active));
  const indent = { paddingInlineStart: 10 + depth * 14 };

  if (!hasKids) {
    return (
      <li>
        <Link
          href={node.href}
          className="navdrawer__link"
          style={indent}
          aria-current={lit ? "page" : undefined}
          data-active={lit ? "true" : undefined}
          onClick={onClose}
        >
          {node.label}
        </Link>
      </li>
    );
  }

  const panelId = `navd-${node.id}`;
  return (
    <li>
      <div className="navdrawer__row" style={indent}>
        {node.href ? (
          <>
            <Link
              href={node.href}
              className="navdrawer__link navdrawer__link--grow"
              aria-current={active(node.href) ? "page" : undefined}
              data-active={lit ? "true" : undefined}
              onClick={onClose}
            >
              {node.label}
            </Link>
            <button
              type="button"
              className="navdrawer__expand"
              aria-expanded={isOpen}
              aria-controls={panelId}
              aria-label={node.label}
              onClick={() => onToggle(node.id)}
            >
              <span aria-hidden>{isOpen ? "▾" : "▸"}</span>
            </button>
          </>
        ) : (
          <button
            type="button"
            className="navdrawer__link navdrawer__link--grow"
            aria-expanded={isOpen}
            aria-controls={panelId}
            data-active={lit ? "true" : undefined}
            onClick={() => onToggle(node.id)}
          >
            {node.label}
            <span className="navdrawer__caret" aria-hidden>
              {isOpen ? "▾" : "▸"}
            </span>
          </button>
        )}
      </div>
      <ul id={panelId} className="navdrawer__sub" hidden={!isOpen}>
        {node.children.map((c) => (
          <DrawerNode
            key={c.id}
            node={c}
            depth={depth + 1}
            openIds={openIds}
            active={active}
            onToggle={onToggle}
            onClose={onClose}
          />
        ))}
      </ul>
    </li>
  );
}

export function NavDrawer({
  open,
  onClose,
  primary,
  groups,
  summary,
}: {
  open: boolean;
  onClose: () => void;
  primary: NavItem[];
  groups: NavGroup[];
  /** 用户摘要（用户名/等级/上传/魔力）；null = 未登录 → 游客文案 */
  summary: {
    username: string;
    className: string;
    uploaded: string;
  } | null;
}) {
  const pathname = usePathname();
  const { dict } = useI18n();
  const drawerRef = useRef<HTMLDivElement>(null);
  const t = dict.navDrawer;
  // 必须 portal 到 body：本组件由 MobileNavShell 渲染在顶栏 <header
  // className="tide-header"> 里，而头栏带 backdrop-filter（theme-tide.css:468）
  // ——它成了 fixed 后代的包含块，`.navdrawer-root{inset:0}` 于是贴着 72px 高的
  // 头栏求解，抽屉被关进头栏条带（2026-10-16 弹窗普查 A5）。
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  // Tab 不再穿透到抽屉背后的页面（此前只有 Esc 与滚动锁两项）
  useFocusTrap(open, drawerRef);

  const tree = useMemo(
    () => buildNavTree(primary, groups, dict.nav.more),
    [primary, groups, dict.nav.more],
  );
  // 选中态 query 部分：挂载后读真实地址（避开 useSearchParams 的 Suspense 要求）
  const [search, setSearch] = useState("");
  useEffect(() => {
    setSearch(window.location.search);
  }, [pathname]);
  const sp = useMemo(() => new URLSearchParams(search), [search]);
  const claimed = useMemo(
    () => claimedPaths(leavesOf(tree), sp),
    [tree, sp],
  );
  const active = useCallback(
    (href: string) => isActive(href, pathname, sp, claimed),
    [pathname, sp, claimed],
  );
  const [openIds, setOpenIds] = useState<Set<string>>(new Set());
  const toggle = useCallback((id: string) => {
    setOpenIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
  // 打开时自动展开当前页所在链（首屏就能看到「你在这里」）
  useEffect(() => {
    if (!open) return;
    setOpenIds(
      new Set(pathToActive(tree, (n) => n.href !== "" && active(n.href))),
    );
  }, [open, tree, active]);

  // 路由变化自动收起
  useEffect(() => {
    onClose();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathname]);

  // ESC / body 滚动锁
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prevOverflow;
    };
  }, [open, onClose]);

  // 右缘手势关闭（pointerdown 起手即收，无需完整 swipe 实现）
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (window.innerWidth - e.clientX <= 48) onClose();
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [open, onClose]);

  if (!open) return null;
  if (!mounted) return null;
  return createPortal(
    <div
      className="navdrawer-root"
      role="dialog"
      aria-modal="true"
      aria-label={t.open}
    >
      <div className="navdrawer-mask" onClick={() => onClose()} />
      <div className="navdrawer" ref={drawerRef}>
        <div className="navdrawer__head">
          <div className="navdrawer__user">
            <span className="navdrawer__avatar" aria-hidden>
              {summary ? summary.username.slice(0, 1).toUpperCase() : "·"}
            </span>
            <div>
              <p className="navdrawer__name">
                {summary ? summary.username : t.guest}
              </p>
              {summary && (
                <p className="navdrawer__stat">
                  {summary.className} · ↑ {summary.uploaded}
                </p>
              )}
            </div>
          </div>
          <button
            type="button"
            className="navdrawer__close"
            aria-label={t.close}
            onClick={() => onClose()}
          >
            ✕
          </button>
        </div>
        {/* 搜索（与桌面顶栏同源 GlobalSearch 弹窗形态）：2026-10-31 起
            顶栏按钮 <md 也常驻，抽屉这枚是双保险入口。 */}
        <div className="navdrawer__search">
          <GlobalSearch
            placeholder={dict.common.globalSearchPh}
            tipTorrents={dict.common.gsTorrents}
            tipTopics={dict.common.gsTopics}
            tipEmpty={dict.common.gsEmpty}
            tipMore={dict.common.gsMore}
            tipSearching={dict.common.gsSearching}
            tipError={dict.common.gsError}
            retryLabel={dict.common.gsRetry}
            searchLabel={dict.common.gsOpen}
          />
        </div>
        <nav className="navdrawer__body" aria-label={dict.nav.ariaPrimary}>
          <p className="navdrawer__gtitle">{dict.nav.ariaPrimary}</p>
          <ul className="navdrawer__tree">
            {tree.map((n) => (
              <DrawerNode
                key={n.id}
                node={n}
                depth={0}
                openIds={openIds}
                active={active}
                onToggle={toggle}
                onClose={onClose}
              />
            ))}
          </ul>
        </nav>
        {/* 主题切换 <md 从顶栏收进来（280px 外屏态右工具区挤不下） */}
        <div className="navdrawer__foot">
          <ThemeToggle label={dict.common.themeToggle} />
        </div>
      </div>
    </div>,
    document.body,
  );
}

/** 汉堡触发按钮（受控）：44px 触控目标 */
export function NavDrawerTrigger({
  onClick,
  label,
}: {
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      className="navdrawer-trigger"
      aria-label={label}
      aria-haspopup="dialog"
      onClick={onClick}
    >
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      >
        <path d="M4 7h16M4 12h16M4 17h10" />
      </svg>
    </button>
  );
}
