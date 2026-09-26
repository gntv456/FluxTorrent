"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useMediaQuery } from "@/lib/hooks/use-media";

/** 平板细侧栏（M4，方案 §8）：md–lg 区间左侧 172px 图标+短名导航，
 *  底 Tab 同时隐藏（CSS 控制）。条目与底 Tab 同源一级项（调用方注入），
 *  不引入第三份配置。≥lg 不渲染（桌面顶栏接管），<md 不渲染（底 Tab 接管）。 */
export function TabletSidebar({
  items,
  ariaLabel,
}: {
  items: { href: string; label: string; icon: React.ReactNode }[];
  ariaLabel: string;
}) {
  const pathname = usePathname();
  const medium = useMediaQuery(
    "(min-width: 768px) and (max-width: 1023px)",
    false,
  );
  if (!medium) return null;
  return (
    <nav className="tsidebar" aria-label={ariaLabel}>
      {items.map((n) => {
        const active =
          n.href === "/"
            ? pathname === "/"
            : pathname === n.href || pathname.startsWith(`${n.href}/`);
        return (
          <Link
            key={n.href}
            href={n.href}
            className={`tsidebar__item ${active ? "is-active" : ""}`}
          >
            {n.icon}
            <span className="tsidebar__label">{n.label}</span>
          </Link>
        );
      })}
    </nav>
  );
}
