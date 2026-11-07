"use client";

import Link from "next/link";
import { useState } from "react";
import type { NavItem } from "@/lib/nav-menu";

/** 主菜单「教育资源」级联三列面板（0474）：
 *  四级结构不再平铺（学段×年级×科目 ~200 链接的墙），改**级联逐级展开**——
 *  左列学段（hover 切换）→ 中列年级 → 右列科目；每列首项为该级「全部」。
 *  触屏：点按切列，科目为 Link 直达。数据来自 nav-edu 的 eduPrimary sub。 */

export function MainMenuEduDrop({
  sub,
  isActive,
  topHref,
}: {
  sub: NonNullable<NavItem["sub"]>;
  isActive: (href: string) => boolean;
  topHref: string;
}) {
  const [stage, setStage] = useState(0);
  const [grade, setGrade] = useState(0);
  const st = sub[stage] ?? sub[0];
  const gItem = st?.items[grade] ?? st?.items[0];
  const colCls = "mainmenu-edu__col";
  return (
    <div className="mainmenu-drop mainmenu-edu">
      <div className={colCls}>
        <Link href={topHref} className="mainmenu-edu__all">
          全部教育资源
        </Link>
        {sub.map((s, si) => (
          <button
            key={s.href + s.label}
            type="button"
            className={
              "mainmenu-edu__row" + (si === stage ? " is-on" : "")
            }
            onMouseEnter={() => {
              setStage(si);
              setGrade(0);
            }}
            onClick={() => {
              setStage(si);
              setGrade(0);
            }}
          >
            <span>{s.label}</span>
            <span aria-hidden>›</span>
          </button>
        ))}
      </div>
      <div className={colCls}>
        <Link href={st?.href ?? topHref} className="mainmenu-edu__all">
          全部{st?.label}
        </Link>
        {(st?.items ?? []).map((g, gi) =>
          g.children ? (
            <button
              key={g.href + g.label}
              type="button"
              className={
                "mainmenu-edu__row" + (gi === grade ? " is-on" : "")
              }
              onMouseEnter={() => setGrade(gi)}
              onClick={() => setGrade(gi)}
            >
              <span>{g.label}</span>
              <span aria-hidden>›</span>
            </button>
          ) : (
            <Link
              key={g.href + g.label}
              href={g.href}
              className="mainmenu-edu__row"
              data-active={isActive(g.href) ? "true" : undefined}
            >
              <span>{g.label}</span>
            </Link>
          ),
        )}
      </div>
      <div className={colCls}>
        {gItem && (
          <>
            <Link href={gItem.href} className="mainmenu-edu__all">
              全部{gItem.label}
            </Link>
            {(gItem.children ?? []).map((s) => (
              <Link
                key={s.href + s.label}
                href={s.href}
                className="mainmenu-edu__row"
                data-active={isActive(s.href) ? "true" : undefined}
              >
                <span>{s.label}</span>
              </Link>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
