/**
 * 在线玩家头像圈（演示占位）。
 *
 * 原型为一排真实玩家头像；当前房间口尚未返回在线/本期下注玩家列表，
 * 这里先用首字母 + 彩色底的静态演示占位排满视觉位，字段结构预留好，
 * 后续接真实玩家接口时只换数据源、不动 DOM 结构。
 */

import { useI18n } from "@/i18n/client";

interface DemoPlayer {
  name: string;
  color: string;
}

const DEMO_COLORS = [
  "#d84343",
  "#3f76c9",
  "#3d9a63",
  "#8a5fc9",
  "#c98a3f",
  "#4a9e9e",
  "#b4557f",
];

function initial(name: string): string {
  return name.trim().charAt(0).toUpperCase();
}

export function PlayerRail() {
  const { dict } = useI18n();
  const names = dict.games.bigsmall.roomDemoNames.split("|");
  return (
    <div className="bs-rail" aria-label={dict.games.bigsmall.roomPlayersLabel}>
      {names.map((name, i) => (
        <div className="bs-rail-player" key={name}>
          <span
            className="bs-rail-avatar"
            style={{ background: DEMO_COLORS[i % DEMO_COLORS.length] }}
            aria-hidden
          >
            {initial(name)}
          </span>
          <span className="bs-rail-name">{name}</span>
        </div>
      ))}
    </div>
  );
}
