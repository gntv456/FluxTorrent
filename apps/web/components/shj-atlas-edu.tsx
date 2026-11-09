"use client";

import Link from "next/link";
import { useState } from "react";
import { useI18n } from "@/i18n/client";
import type { NavItem } from "@/lib/nav-menu";

/** 山海经侧栏「教育资源」钻取视图（0474）：
 *  四级结构不再平铺塞进侧栏（缩进层级深、可读性差），改为**整屏下探**——
 *  第 1 页学段 → 第 2 页年级 → 第 3 页科目，每页带 ← 返回，App 式交互。
 *  由 shj-atlas.tsx 在 eduDrill 激活时整块替换导航列表；路由变化即关闭。 */

type Stage = NonNullable<NavItem["sub"]>[number];

export function ShjAtlasEduDrill(props: {
  /** 教育资源的 sub（5 个学段组，items=年级，年级 children=科目） */
  sub: NonNullable<NavItem["sub"]>;
  /** 与顶栏/抽屉同一套选中口径（父级注入） */
  active: (href: string) => boolean;
  /** 回主菜单（点返回到根/路由变化时由父级关闭） */
  onClose: () => void;
}) {
  const { sub, active, onClose } = props;
  const { dict } = useI18n();
  // 钻取游标：stage = 学段下标；grade = 年级在 stage.items 里的下标（null=年级页）
  const [cursor, setCursor] = useState<{ stage: number; grade: number | null }>(
    { stage: -1, grade: null },
  );

  const stage = sub[cursor.stage];
  const grade =
    cursor.grade != null && stage ? stage.items[cursor.grade] : null;

  const BackRow = ({ onClick, title }: { onClick: () => void; title: string }) => (
    <button type="button" className="shj-atlas__drillback" onClick={onClick}>
      <span aria-hidden>‹</span>
      <span>{dict.nav.eduBack}</span>
      <b>{title}</b>
    </button>
  );

  // 第 3 页：科目直达
  if (stage && grade) {
    return (
      <div className="shj-atlas__drill">
        <BackRow
          onClick={() => setCursor({ stage: cursor.stage, grade: null })}
          title={grade.label}
        />
        <ul className="shj-atlas__list">
          <li>
            <Link
              href={grade.href}
              className="shj-atlas__item"
              aria-current={active(grade.href) ? "page" : undefined}
            >
              {dict.nav.eduAllGrade}
            </Link>
          </li>
          {(grade.children ?? []).map((s) => (
            <li key={s.href + s.label}>
              <Link
                href={s.href}
                className="shj-atlas__item"
                aria-current={active(s.href) ? "page" : undefined}
              >
                {s.label}
              </Link>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  // 第 2 页：年级列表
  if (stage) {
    return (
      <div className="shj-atlas__drill">
        <BackRow onClick={() => setCursor({ stage: -1, grade: null })} title={stage.label} />
        <ul className="shj-atlas__list">
          <li>
            <Link
              href={stage.href}
              className="shj-atlas__item"
              aria-current={active(stage.href) ? "page" : undefined}
            >
              {dict.nav.eduAllStage}
            </Link>
          </li>
          {stage.items.map((g, gi) => (
            <li key={g.href + g.label}>
              {g.children ? (
                <button
                  type="button"
                  className="shj-atlas__item shj-atlas__drillrow"
                  onClick={() => setCursor({ stage: cursor.stage, grade: gi })}
                >
                  <span>{g.label}</span>
                  <span className="shj-atlas__drillmore" aria-hidden>
                    ›
                  </span>
                </button>
              ) : (
                <Link
                  href={g.href}
                  className="shj-atlas__item"
                  aria-current={active(g.href) ? "page" : undefined}
                >
                  {g.label}
                </Link>
              )}
            </li>
          ))}
        </ul>
      </div>
    );
  }

  // 第 1 页：学段列表
  return (
    <div className="shj-atlas__drill">
      <BackRow onClick={onClose} title={dict.nav.eduRes} />
      <ul className="shj-atlas__list">
        <li>
          <Link
            href="/torrents?category_id=13"
            className="shj-atlas__item"
            aria-current={
              active("/torrents?category_id=13") ? "page" : undefined
            }
          >
            {dict.nav.eduAllRes}
          </Link>
        </li>
        {sub.map((st, si) => (
          <li key={st.href + st.label}>
            <button
              type="button"
              className="shj-atlas__item shj-atlas__drillrow"
              onClick={() => setCursor({ stage: si, grade: null })}
            >
              <span>{st.label}</span>
              <span className="shj-atlas__drillmore" aria-hidden>
                ›
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
