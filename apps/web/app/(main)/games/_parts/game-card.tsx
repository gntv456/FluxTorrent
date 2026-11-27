//! 大厅卡面渲染（加速批八期 2026-11-27 从 page.tsx 拆出，行数门清偿）。
//!
//! server 组件：吃 page 侧算好的纯展示 props（badge 文案/锁定等级/条目），
//! 不含取数。每卡按 live/locked 二分：<Link> 可玩 / <div> 置灰。

import Link from "next/link";
import { fmt } from "@/i18n/config";
import type { GameEntry } from "@/lib/games-registry";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { SCENES } from "@/lib/game-scenes";

export interface GameCardStrings {
  hot: string;
  soon: string;
  play: string;
  locked: string;
}

export function GameCard({
  e,
  badge,
  locked,
  title,
  sub,
  strings,
}: {
  e: GameEntry;
  /** 角标文案（page 侧按后端真实赔率/奖池算好）；null = 不显示 */
  badge: string | null;
  /** 等级门槛；0 = 不锁 */
  locked: number;
  /** 标题（{magic} 占位符已按站点货币名替换） */
  title: string;
  sub: string;
  strings: GameCardStrings;
}) {
  const Scene = SCENES[e.key];
  const inner = (
    <>
      <div className={`gc-art gc-tone-${e.tone}`}>
        {Scene && <Scene className="gc-scene" />}
        {!Scene && (
          <span className="gc-glyph" aria-hidden>
            <ArcadeGlyph k={e.key} />
          </span>
        )}
        {e.hot && e.live && !locked && (
          <span className="gc-hot">{strings.hot}</span>
        )}
        {badge && e.live && !locked && (
          <span className="gc-badge num">{badge}</span>
        )}
        {locked > 0 && (
          <span className="gc-soon num">
            {fmt(strings.locked, { n: locked })}
          </span>
        )}
        {!e.live && <span className="gc-soon">{strings.soon}</span>}
      </div>
      <div className="gc-body">
        <h3 className="gc-title">{title}</h3>
        <p className="gc-sub">{sub}</p>
        <span className="gc-go">
          {strings.play}
          <i aria-hidden>→</i>
        </span>
      </div>
    </>
  );
  return e.live && !locked ? (
    <Link href={e.href} prefetch={false} className="gc-card" aria-label={title}>
      {inner}
    </Link>
  ) : (
    <div key={e.key} className="gc-card is-soon" aria-disabled="true">
      {inner}
    </div>
  );
}
