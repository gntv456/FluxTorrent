"use client";

/** 主题面板的淡彩三组色卡（fresh/ink/dream）+ 自选颜色卡。
 *  从 theme-toggle.tsx 拆出（行数门禁，2026-11-14）；渲染契约：
 *  网格按钮 role=menuitemradio、选中勾、色卡预览类 .thsw--<id>（usermenu.css
 *  手写渐变）都与拆出前逐字一致。自选卡接原生 <input type="color">，
 *  onChange 上抛给 ThemeToggle.pickCustom（应用+持久化在那边统一做）。 */

import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";
import type { ThemeChoice, ThemeGroup, ThemeId } from "@/lib/themes";

export function PastelGroups({
  groups,
  byGroup,
  choice,
  customColor,
  nameOf,
  groupName,
  customLabel,
  isActive,
  onPick,
  onPickCustom,
}: {
  groups: ThemeGroup[];
  byGroup: (g: ThemeGroup) => ThemeId[];
  choice: ThemeChoice;
  customColor: string | null;
  nameOf: (id: ThemeId) => string;
  groupName: (g: ThemeGroup) => string;
  /** 自选颜色卡文案（dict.common.themeCustom，由父组件取好传入） */
  customLabel: string;
  /** 色卡 active 判断（明暗家族口径：暗色系统落 -night 孪生也亮勾） */
  isActive: (id: ThemeId) => boolean;
  onPick: (id: ThemeId) => void;
  onPickCustom: (hex: string) => void;
}) {
  return (
    <>
      {groups
        .filter((g) => g !== "classic")
        .map((g) => (
          <div key={g} className="thmenu__grp">
            <div className="thmenu__grp-name">{groupName(g)}</div>
            <div className="thmenu__grid" role="group">
              {byGroup(g).map((id) => {
                const active = isActive(id);
                return (
                  <button
                    key={id}
                    type="button"
                    role="menuitemradio"
                    aria-checked={active}
                    title={nameOf(id)}
                    className={`thcard${active ? " is-active" : ""}`}
                    onClick={() => onPick(id)}
                  >
                    <span className={`thcard__sw thsw--${id}`} aria-hidden />
                    {active ? (
                      <Icon name="check" size={13} className="thcard__check" />
                    ) : null}
                    <span className="thcard__name">{nameOf(id)}</span>
                  </button>
                );
              })}
              {/* 自选颜色卡：dream 组末位（QQ 口径），点卡片弹原生取色器 */}
              {g === "dream" ? (
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={choice === "custom"}
                  title={customLabel}
                  className={`thcard thcard--custom${
                    choice === "custom" ? " is-active" : ""
                  }`}
                  onClick={() => {
                    const input =
                      document.querySelector<HTMLInputElement>(
                        ".thcard__picker",
                      );
                    input?.click();
                  }}
                >
                  <span className="thcard__sw thcard__sw--custom" aria-hidden>
                    <span className="thcard__sw-c1" />
                    <span className="thcard__sw-c2" />
                    <span className="thcard__sw-c3" />
                    <input
                      type="color"
                      className="thcard__picker"
                      value={
                        customColor ??
                        // design-cov-exempt: input[type=color] 平台字面量
                        "#3A8FE0"
                      }
                      onChange={(e) => onPickCustom(e.target.value)}
                    />
                  </span>
                  {choice === "custom" ? (
                    <Icon name="check" size={13} className="thcard__check" />
                  ) : null}
                  <span className="thcard__name">{customLabel}</span>
                </button>
              ) : null}
            </div>
          </div>
        ))}
    </>
  );
}
