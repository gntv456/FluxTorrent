"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { Forum } from "@fluxtorrent/domain-types";

/** 版块导航（桌面左列 / 手机「版块」弹窗共用）。
 *
 *  高亮按 pathname 现算，不再由页面传 activeId：三栏壳已提升为
 *  (shell)/layout.tsx，切换版块时布局实例不重建、服务端拿不到新
 *  id，只有客户端 usePathname 才能跟着路由更新选中项。 */

export interface BoardGroup {
  key: string;
  name: string;
  hidden: boolean;
  items: Forum[];
}

/** 版块导航圆点色板（样图 f3b-nav__item i 的六色，按版块 id 取）。
 *  P2-5 令牌化：色值真源在 base.css --fhue-*（固定 hue 轮换，
 *  明暗同值，口径见该处注释）。 */
const NAV_DOTS = [
  "var(--fhue-sky)",
  "var(--fhue-mint)",
  "var(--fhue-sun)",
  "var(--fhue-indigo)",
  "var(--fhue-candy)",
  "var(--fhue-coral)",
];

export function ForumNav({
  groups,
  totalTopics,
  labels,
}: {
  groups: BoardGroup[];
  totalTopics: number;
  labels: {
    title: string;
    allNodes: string;
    navAll: string;
    hiddenTag: string;
  };
}) {
  const pathname = usePathname() ?? "";
  const m = /^\/forums\/(\d+)\/?$/.exec(pathname);
  const activeId = m ? Number(m[1]) : undefined;
  return (
    <aside className="pting-sidenav">
      <h2>{labels.allNodes}</h2>
      <nav aria-label={labels.title}>
        {/* 一级：全部版块（样图该项没有圆点，选中态 sky-soft） */}
        <Link
          href="/forums"
          className={activeId === undefined ? "on" : undefined}
        >
          {labels.navAll}
          <span className="fh-nav__cnt num">{totalTopics}</span>
        </Link>
        {groups.map((g) => (
          <div key={g.key}>
            <div className="fh-nav__group">
              {g.name}
              {g.hidden && (
                <span className="pting-card__count">
                  {labels.hiddenTag}
                </span>
              )}
            </div>
            {g.items.map((f) => (
              <Link
                key={f.id}
                href={`/forums/${f.id}`}
                className={[
                  "fh-nav__sub",
                  g.hidden ? "fh-nav__hidden" : "",
                  activeId === f.id ? "on" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                {/* 二级圆点（样图 f3b-nav__item i：6px 实心圆，
                    按版块 id 取色板） */}
                <span
                  className="fh-nav__dot"
                  style={{
                    background: NAV_DOTS[f.id % NAV_DOTS.length],
                  }}
                  aria-hidden="true"
                />
                {f.name}
                <span className="fh-nav__cnt num">{f.topics}</span>
              </Link>
            ))}
          </div>
        ))}
      </nav>
    </aside>
  );
}
