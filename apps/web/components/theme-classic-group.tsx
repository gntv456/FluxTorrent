"use client";

/** 主题面板的经典组（4 档 + auto，行式选项）。
 *  从 theme-toggle.tsx 拆出（行数门禁，2026-11-14）；行式 .thmenu__opt +
 *  小色点 .thsw 的渲染契约与拆出前逐字一致。 */

import { Icon } from "@/components/icons";
import { THEMES, type ThemeChoice, type ThemeId } from "@/lib/themes";

export function ClassicGroup({
  ids,
  choice,
  nameOf,
  autoName,
  classicLabel,
  onPick,
}: {
  ids: ThemeId[];
  choice: ThemeChoice;
  nameOf: (id: ThemeId) => string;
  autoName: string;
  /** 组名「经典」（dict.common.themeGroupClassic，父侧取词传入） */
  classicLabel: string;
  onPick: (choice: ThemeChoice) => void;
}) {
  return (
    <>
      <div className="thmenu__grp-name">{classicLabel}</div>
      {ids.map((id) => {
        const active = choice === id;
        return (
          <button
            key={id}
            type="button"
            role="menuitemradio"
            aria-checked={active}
            className={`thmenu__opt${active ? " is-active" : ""}`}
            onClick={() => onPick(id)}
          >
            <span className={`thsw thsw--${id}`} aria-hidden />
            <Icon
              name={THEMES[id]?.scheme === "dark" ? "moon" : "sun"}
              size={15}
            />
            <span className="thmenu__name">{nameOf(id)}</span>
            {active ? (
              <Icon name="check" size={14} className="thmenu__check" />
            ) : null}
          </button>
        );
      })}
      <button
        type="button"
        role="menuitemradio"
        aria-checked={choice === "auto"}
        className={`thmenu__opt${choice === "auto" ? " is-active" : ""}`}
        onClick={() => onPick("auto")}
      >
        <span className="thsw thsw--auto" aria-hidden />
        <Icon name="auto" size={15} />
        <span className="thmenu__name">{autoName}</span>
        {choice === "auto" ? (
          <Icon name="check" size={14} className="thmenu__check" />
        ) : null}
      </button>
    </>
  );
}
