"use client";

/** 主菜单（2026-11-09 定稿）：一级平铺 + 传统 hover 级联下拉。
 *
 *  桌面（≥lg）：带子级的一级条悬停正下弹第一层，子级悬停右侧飞出，
 *  任意层级嵌套（形态参考经典纯 CSS 多级下拉，components/nav-dropdown.tsx）。
 *  顶栏一级仍保留 Link 直进（点击=进该频道首页，悬停=浏览子级）——
 *  hover 浏览与点击直达并存，不再互斥。空 href 的分组名（「更多」）渲染
 *  为 button，只展开不下跳。
 *
 *  <lg 不再渲染本组件（抽屉承载全部同源入口，见 nav-drawer.tsx）。
 *  选中态仍用 lib/nav-tree.ts 的键值全等口径（2026-11-03 定稿）。
 */
import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { NavGroup, NavItem } from "@/lib/nav-menu";
import type { NavNode } from "@/lib/nav-tree";
import {
  buildNavTree,
  claimedPaths,
  isActive as nodeIsActive,
  leavesOf,
} from "@/lib/nav-tree";
import { NavDropList } from "@/components/nav-dropdown";

type Props = {
  items: NavItem[];
  groups?: NavGroup[];
  moreLabel?: string;
  ariaLabel: string;
};

export function MainMenu({
  items,
  groups,
  moreLabel,
  ariaLabel,
}: Props) {
  const pathname = usePathname();
  const sp = useSearchParams();

  const tree = useMemo(
    () => buildNavTree(items, groups ?? [], moreLabel),
    [items, groups, moreLabel],
  );
  const claimed = useMemo(
    () => claimedPaths(leavesOf(tree), sp),
    [tree, sp],
  );
  const active = useCallback(
    (href: string) => nodeIsActive(href, pathname, sp, claimed),
    [pathname, sp, claimed],
  );
  const parentActive = useCallback(
    (n: NavNode) =>
      active(n.href) ||
      leavesOf(n.children).some((c) => active(c.href)),
    [active],
  );

  // 展开态：expanded=指针/焦点停留的一级项，pinned=点击展开的分组名。
  // suppressed=Esc 收起后的抑制位，避免鼠标仍悬停时 :hover 又把面板弹开。
  const [expanded, setExpanded] = useState<string | null>(null);
  const [pinned, setPinned] = useState<string | null>(null);
  const [suppressed, setSuppressed] = useState(false);
  const suppressRef = useRef(false);
  const wrapRef = useRef<HTMLElement>(null);

  const release = useCallback(() => {
    if (!suppressRef.current) return;
    suppressRef.current = false;
    setSuppressed(false);
  }, []);

  // Esc 收起（一份监听）+ 点外部收起，焦点回落触发元素
  useEffect(() => {
    const onDoc = (e: PointerEvent) => {
      if (wrapRef.current?.contains(e.target as Node)) return;
      setExpanded(null);
      setPinned(null);
      release();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const id = pinned ?? expanded;
      if (!id) return;
      e.preventDefault();
      suppressRef.current = true;
      setSuppressed(true);
      setExpanded(null);
      setPinned(null);
      wrapRef.current
        ?.querySelector<HTMLElement>(`[data-nav-trigger="${id}"]`)
        ?.focus();
    };
    document.addEventListener("pointerdown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [expanded, pinned, release]);

  return (
    <nav
      aria-label={ariaLabel}
      ref={wrapRef}
      className="nvc-anchor"
      data-suppressed={suppressed ? "true" : undefined}
      onKeyDown={(e) => {
        if (e.key === "Tab") release();
      }}
      onPointerMove={release}
    >
      <ul className="mainmenu">
        {tree.map((n, i) => {
          const hasKids = n.children.length > 0;
          const openNow = pinned === n.id || expanded === n.id;
          const exact = active(n.href);
          const body = (
            <>
              {n.label}
              {hasKids && (
                <span className="nd-caret" aria-hidden>▾</span>
              )}
            </>
          );
          const common = {
            className: "mainmenu-link",
            "data-active": parentActive(n) ? "true" : undefined,
            "data-nav-trigger": n.id,
          } as const;
          return (
            <li
              key={n.id}
              className={`relative nd-has${openNow ? " is-open" : ""}`}
              data-open={openNow ? "true" : undefined}
              onPointerEnter={() => {
                if (!hasKids) return;
                release();
                setExpanded(n.id);
              }}
              onPointerLeave={() =>
                setExpanded((prev) => (prev === n.id ? null : prev))
              }
              onFocusCapture={() => {
                if (!suppressRef.current && hasKids) setExpanded(n.id);
              }}
              onBlurCapture={(e) => {
                if (e.currentTarget.contains(e.relatedTarget as Node)) return;
                setExpanded((prev) => (prev === n.id ? null : prev));
              }}
            >
              {hasKids && !n.href ? (
                <button
                  type="button"
                  {...common}
                  aria-haspopup="true"
                  aria-expanded={openNow}
                  onClick={() => {
                    if (pinned === n.id) {
                      setPinned(null);
                      setExpanded(null);
                    } else {
                      release();
                      setPinned(n.id);
                      setExpanded(n.id);
                    }
                  }}
                >
                  {body}
                </button>
              ) : (
                <Link
                  href={n.href || "/"}
                  {...common}
                  aria-haspopup={hasKids || undefined}
                  aria-expanded={hasKids ? openNow : undefined}
                  aria-current={exact ? "page" : undefined}
                >
                  {body}
                </Link>
              )}
              {hasKids && (
                <NavDropList
                  node={n}
                  active={active}
                  onNavigate={() => {
                    setExpanded(null);
                    setPinned(null);
                  }}
                  flip={i === tree.length - 1}
                />
              )}
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
