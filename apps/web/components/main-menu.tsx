"use client";

/** 主菜单（2026-11-07 重构）：一级平铺 + 点击逐级展开的导航画布。
 *
 *  三套历史下拉（「更多」网格 / 资源库分组 / 教育三列级联）归一成
 *  一棵 NavNode 树，按主题选渲染壳：
 *    brand / brand-night → NavColumns（横向逐级列）
 *    shanhaijing 两档    → NavScroll（纵向卷轴 + 册页宫格）
 *  选中态仍用 lib/nav-tree.ts 的键值全等口径（2026-11-03 定稿）。
 *
 *  平板收敛（<lg）沿用旧口径：一级收进「更多」，点开后从第一列逐级点。
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

type Props = {
  items: NavItem[];
  groups?: NavGroup[];
  moreLabel?: string;
  ariaLabel: string;
  /** 面板状态条「进入当前层」文案（dict.nav.goCurrent） */
  goLabel?: string;
};

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
  const [open, setOpen] = useState(false);
  const [rootId, setRootId] = useState<string | null>(null);
  const wrapRef = useRef<HTMLElement>(null);

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
    (href: string) => {
      setOpen(false);
      router.push(href);
    },
    [router],
  );

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
    // 再点同一一级钮 = 收起（toggle）：aria-expanded 与操作语义一致
    if (open && rootId === id) {
      setOpen(false);
      return;
    }
    setRootId(id);
    setOpen(true);
  };
  const canvas = {
    tree,
    open,
    rootId,
    active,
    onClose: () => setOpen(false),
    onNavigate: navigate,
    goLabel,
  };

  return (
    <nav aria-label={ariaLabel} ref={wrapRef} className="nvc-anchor">
      <ul className="mainmenu">
        {!compact &&
          tree.map((n) => (
            <li key={n.id} className="relative">
              {n.children.length ? (
                <button
                  type="button"
                  className="mainmenu-link"
                  data-active={parentActive(n) ? "true" : undefined}
                  aria-expanded={open && rootId === n.id}
                  aria-haspopup="true"
                  onClick={() => openAt(n.id)}
                >
                  {n.label} ▾
                </button>
              ) : (
                <Link
                  href={n.href}
                  className="mainmenu-link"
                  data-active={active(n.href) ? "true" : undefined}
                >
                  {n.label}
                </Link>
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
      {scrollShell ? (
        <NavScroll {...canvas} />
      ) : (
        <NavColumns {...canvas} />
      )}
    </nav>
  );
}
