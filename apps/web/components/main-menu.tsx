"use client";

/** 主菜单（2026-11-09 定稿）：一级平铺 + 传统 hover 级联下拉。
 *
 *  桌面（≥lg）：带子级的一级条悬停正下弹第一层，子级悬停右侧飞出，
 *  任意层级嵌套（形态参考经典纯 CSS 多级下拉，components/nav-dropdown.tsx）。
 *  顶栏一级仍保留 Link 直进（点击=进该频道首页，悬停=浏览子级）——
 *  hover 浏览与点击直达并存，不再互斥。
 *
 *  平板收敛（<lg）沿用画布面板：一级收进「更多」，点开逐级钻。
 *  选中态仍用 lib/nav-tree.ts 的键值全等口径（2026-11-03 定稿）。
 */
import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMediaQuery } from "@/lib/hooks/use-media";
import type { NavGroup, NavItem } from "@/lib/nav-menu";
import type { NavNode } from "@/lib/nav-tree";
import {
  buildNavTree,
  claimedPaths,
  isActive as nodeIsActive,
  leavesOf,
} from "@/lib/nav-tree";
import { NavColumns, NavScroll } from "@/components/nav-canvas";
import { NavDropList } from "@/components/nav-dropdown";

/** 主题偏好由 no-flash 内联脚本写 <html data-theme>，客户端跟随切换。 */
function useScrollShell() {
  const [scroll, setScroll] = useState(false);
  useEffect(() => {
    const read = () => {
      const t = document.documentElement.getAttribute("data-theme");
      setScroll(t === "shanhaijing" || t === "shanhaijing-light");
    };
    read();
    if (typeof MutationObserver === "undefined") return;
    const mo = new MutationObserver(read);
    mo.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => mo.disconnect();
  }, []);
  return scroll;
}

type Props = {
  items: NavItem[];
  groups?: NavGroup[];
  moreLabel?: string;
  ariaLabel: string;
  /** 面板状态条「进入当前层」文案（dict.nav.goCurrent） */
  goLabel?: string;
};

export function MainMenu({
  items,
  groups,
  moreLabel,
  ariaLabel,
  goLabel = "进入",
}: Props) {
  const pathname = usePathname();
  const sp = useSearchParams();
  const router = useRouter();
  const compact = useMediaQuery("(max-width: 1023px)", false);
  const scrollShell = useScrollShell();

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
  const navigate = useCallback(
    (href: string) => router.push(href),
    [router],
  );
  const [open, setOpen] = useState(false);
  const [rootId, setRootId] = useState<string | null>(null);
  const wrapRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);
  const openAt = (id: string | null) => {
    if (open && rootId === id) {
      setOpen(false);
      return;
    }
    setRootId(id);
    setOpen(true);
  };

  return (
    <nav aria-label={ariaLabel} ref={wrapRef} className="nvc-anchor">
      <ul className="mainmenu">
        {!compact &&
          tree.map((n) => (
            <li key={n.id} className="relative nd-has">
              <Link
                href={n.href || "/"}
                className="mainmenu-link"
                data-active={parentActive(n) ? "true" : undefined}
              >
                {n.label}
                {n.children.length > 0 && (
                  <span className="nd-caret" aria-hidden>▾</span>
                )}
              </Link>
              {n.children.length > 0 && (
                <NavDropList
                  node={n}
                  active={active}
                  onNavigate={navigate}
                />
              )}
            </li>
          ))}
        {compact && (
          <li className="relative">
            <button
              type="button"
              className="mainmenu-link"
              aria-expanded={open}
              aria-haspopup="true"
              onClick={() => openAt(null)}
            >
              {moreLabel ?? "更多 ▾"}
            </button>
          </li>
        )}
      </ul>
      {compact && (
        scrollShell ? (
          <NavScroll
            tree={tree}
            open={open}
            rootId={rootId}
            active={active}
            onClose={() => setOpen(false)}
            onNavigate={(href) => {
              setOpen(false);
              navigate(href);
            }}
            goLabel={goLabel}
          />
        ) : (
          <NavColumns
            tree={tree}
            open={open}
            rootId={rootId}
            active={active}
            onClose={() => setOpen(false)}
            onNavigate={(href) => {
              setOpen(false);
              navigate(href);
            }}
            goLabel={goLabel}
          />
        )
      )}
    </nav>
  );
}
