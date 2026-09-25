"use client";

import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { useEffect, useRef, useState } from "react";

/** 导航（Seedlight §3）：一级平铺 + 「更多 ▾」分组下拉（点击展开，点外部/ESC 收起）。
 *
 *  选中态口径：裸路径条目（资源库 /torrents）与带参条目（官种
 *  /torrents?official=1）共享同一路径——带参条目仅当当前 URL 携带同参数
 *  才点亮；裸路径条目在同路径带参条目命中时「让位」，否则三个条目
 *  （资源库/官种/更多）会一起点亮（参数剥掉后匹配路径必然同真）。 */
type NavItem = { href: string; label: string };
type NavGroup = { group: string; items: NavItem[] };

export function MainMenu({
  items,
  groups,
  moreLabel,
  ariaLabel,
}: {
  items: { href: string; label: string }[];
  /** 「更多 ▾」收纳域分组；缺省则不渲染下拉 */
  groups?: NavGroup[];
  moreLabel?: string;
  ariaLabel: string;
}) {
  const pathname = usePathname();
  const sp = useSearchParams();
  const [moreOpen, setMoreOpen] = useState(false);
  const moreRef = useRef<HTMLLIElement>(null);

  const pathActive = (p: string) =>
    p === "/"
      ? pathname === "/"
      : pathname === p || pathname.startsWith(`${p}/`);
  /** 当前 URL 是否携带该 href query 的全部键值 */
  const queryActive = (href: string) => {
    const q = href.split("?")[1];
    if (!q) return false;
    for (const [k, v] of new URLSearchParams(q)) {
      if (sp.get(k) !== v) return false;
    }
    return true;
  };
  const allItems = [...items, ...(groups ?? []).flatMap((g) => g.items)];
  // 命中的带参条目所占据的路径：裸路径条目在这些路径上让位
  const claimedPaths = new Set(
    allItems
      .filter((i) => queryActive(i.href))
      .map((i) => i.href.split("?")[0]),
  );

  const isActive = (href: string) => {
    const [path, query] = href.split("?");
    if (!pathActive(path)) return false;
    if (query) {
      for (const [k, v] of new URLSearchParams(query)) {
        if (sp.get(k) !== v) return false;
      }
      return true;
    }
    return !claimedPaths.has(path);
  };
  const moreActive = (groups ?? []).some((g) =>
    g.items.some((i) => isActive(i.href)),
  );

  useEffect(() => {
    if (!moreOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (moreRef.current && !moreRef.current.contains(e.target as Node))
        setMoreOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMoreOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [moreOpen]);

  return (
    <nav aria-label={ariaLabel}>
      <ul className="mainmenu">
        {items.map((n) => {
          const external = /^https?:\/\//i.test(n.href);
          return (
            <li key={n.href}>
              {external ? (
                <a
                  href={n.href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="mainmenu-link"
                >
                  {n.label}
                </a>
              ) : (
                <Link
                  href={n.href}
                  className="mainmenu-link"
                  data-active={isActive(n.href) ? "true" : undefined}
                >
                  {n.label}
                </Link>
              )}
            </li>
          );
        })}
        {groups && groups.length > 0 && (
          <li className="relative" ref={moreRef}>
            <button
              type="button"
              className="mainmenu-link"
              data-active={moreActive || moreOpen ? "true" : undefined}
              aria-expanded={moreOpen}
              aria-haspopup="true"
              onClick={() => setMoreOpen((v) => !v)}
            >
              {moreLabel ?? "更多 ▾"}
            </button>
            {moreOpen && (
              <div className="mainmenu-drop">
                {groups.map((g) => (
                  <div key={g.group} className="mainmenu-drop__group">
                    <p className="mainmenu-drop__title">{g.group}</p>
                    <div className="mainmenu-drop__links">
                      {g.items.map((i) => (
                        <Link
                          key={i.href}
                          href={i.href}
                          onClick={() => setMoreOpen(false)}
                          className="mainmenu-drop__link"
                          data-active={isActive(i.href) ? "true" : undefined}
                        >
                          {i.label}
                        </Link>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </li>
        )}
      </ul>
    </nav>
  );
}
