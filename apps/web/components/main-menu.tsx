"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

/** #mainmenu 卡片导航（客户端跟踪当前路径做橙色渐变高亮）。 */
export function MainMenu({
  items,
  ariaLabel,
}: {
  items: { href: string; label: string }[];
  ariaLabel: string;
}) {
  const pathname = usePathname();
  return (
    <nav aria-label={ariaLabel}>
      <ul className="mainmenu">
        {items.map((n) => {
          const path = n.href.split("?")[0];
          const active =
            path === "/"
              ? pathname === "/"
              : pathname === path || pathname.startsWith(`${path}/`);
          return (
            <li key={n.href}>
              <Link
                href={n.href}
                className="mainmenu-link"
                data-active={active ? "true" : undefined}
              >
                {n.label}
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
