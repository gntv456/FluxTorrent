"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef, useState } from "react";

/** 导航（Seedlight §3）：一级平铺 + 「更多 ▾」分组下拉（点击展开，点外部/ESC 收起）。 */
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
  const [moreOpen, setMoreOpen] = useState(false);
  const moreRef = useRef<HTMLLIElement>(null);

  const isActive = (href: string) => {
    const path = href.split("?")[0];
    return path === "/" ? pathname === "/" : pathname === path || pathname.startsWith(`${path}/`);
  };
  const moreActive = (groups ?? []).some((g) => g.items.some((i) => isActive(i.href)));

  useEffect(() => {
    if (!moreOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (moreRef.current && !moreRef.current.contains(e.target as Node)) setMoreOpen(false);
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
                <Link href={n.href} className="mainmenu-link" data-active={isActive(n.href) ? "true" : undefined}>
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
