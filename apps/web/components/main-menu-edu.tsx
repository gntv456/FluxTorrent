"use client";

import Link from "next/link";
import { useState } from "react";
import type { NavItem } from "@/lib/nav-menu";

/** 主菜单「教育资源」经典级联（0478 重做）：
 *  hover 一级（教育资源）→ 弹出二级学段列；hover 学段行 → 右侧滑出
 *  三级年级列；hover 年级行 → 再滑出四级科目列。面板宽度随已展开
 *  层级自适应（flex 容器只含已展开的列），未展开下一级时不显示空列。
 *  每级行 hover 即切换（触屏点按同权），叶子与各级「全部」为 Link 直达。
 *  数据来自 nav-edu 的 eduPrimary sub：sub=学段，items=年级，
 *  年级.children=科目。 */

type Sub = NonNullable<NavItem["sub"]>[number];
type Leaf = NavItem & { children?: NavItem[] };

export function MainMenuEduDrop({
  sub,
  isActive,
  topHref,
}: {
  sub: NonNullable<NavItem["sub"]>;
  isActive: (href: string) => boolean;
  topHref: string;
}) {
  // 已展开的层级：学段下标 / 年级下标；null = 该级未展开
  const [stage, setStage] = useState<number | null>(null);
  const [grade, setGrade] = useState<number | null>(null);
  const st: Sub | undefined = stage != null ? sub[stage] : undefined;
  const grades: Leaf[] = st?.items ?? [];
  const g: Leaf | undefined =
    grade != null ? grades[grade] : undefined;

  const Row = ({
    label,
    href,
    on,
    hasChild,
    onHover,
    onClick,
  }: {
    label: string;
    href?: string;
    on?: boolean;
    hasChild?: boolean;
    onHover?: () => void;
    onClick?: () => void;
  }) => {
    const cls =
      "mainmenu-edu__row" + (on ? " is-on" : "") + (hasChild ? " has-sub" : "");
    if (href && !hasChild) {
      return (
        <Link
          href={href}
          className={cls}
          data-active={isActive(href) ? "true" : undefined}
        >
          <span>{label}</span>
        </Link>
      );
    }
    return (
      <button
        type="button"
        className={cls}
        onMouseEnter={() => onHover?.()}
        onClick={() => onClick?.()}
      >
        <span>{label}</span>
        {hasChild && (
          <span className="mainmenu-edu__caret" aria-hidden>
            ›
          </span>
        )}
      </button>
    );
  };

  return (
    <div className="mainmenu-drop mainmenu-edu">
      {/* 二级：学段列（一级菜单 hover 弹出的第一列） */}
      <div className="mainmenu-edu__col">
        {sub.map((s, si) => (
          <Row
            key={s.href + s.label}
            label={s.label}
            href={s.href}
            hasChild={(s.items ?? []).some((x) => x.children)}
            on={si === stage}
            onHover={() => {
              setStage(si);
              setGrade(null);
            }}
            onClick={() => {
              setStage(si);
              setGrade(null);
            }}
          />
        ))}
      </div>
      {/* 三级：年级列（hover 学段后滑出） */}
      {st && (
        <div className="mainmenu-edu__col">
          <Link href={st.href} className="mainmenu-edu__all">
            全部{st.label}
          </Link>
          {grades.map((g2, gi) => (
            <Row
              key={g2.href + g2.label}
              label={g2.label}
              href={g2.href}
              hasChild={!!g2.children?.length}
              on={gi === grade}
              onHover={() => setGrade(gi)}
              onClick={() => setGrade(gi)}
            />
          ))}
        </div>
      )}
      {/* 四级：科目列（hover 年级后滑出；无年级层的学段直接科目） */}
      {g && !!g.children?.length && (
        <div className="mainmenu-edu__col">
          <Link href={g.href} className="mainmenu-edu__all">
            全部{g.label}
          </Link>
          {g.children.map((s) => (
            <Row key={s.href + s.label} label={s.label} href={s.href} />
          ))}
        </div>
      )}
      {/* 无年级层的学段（大学）：学段列展开后直接科目列 */}
      {st && !grades.some((x) => x.children?.length) && (
        <div className="mainmenu-edu__col">
          {grades.map((g2) => (
            <Row key={g2.href + g2.label} label={g2.label} href={g2.href} />
          ))}
        </div>
      )}
      {/* 面板留白基线：未展开任何学段时给个最小宽，避免一级 hover 闪缩 */}
      {st === undefined && (
        <div className="mainmenu-edu__col mainmenu-edu__col--hint" />
      )}
      <span className="sr-only">{topHref}</span>
    </div>
  );
}
