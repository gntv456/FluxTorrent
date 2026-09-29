"use client";

/**
 * 娱乐屋周榜：票根收集榜 + 本周局数榜。
 *
 * 排名口径由后端定死：**只按收集度与局数，绝不按净输赢**——按输赢排榜等于
 * 公开鼓励玩家加注，和「四玩法一律回收魔力」对着干。这里只负责呈现。
 */

import { useI18n } from "@/i18n/client";

export interface ArcadeBoardRow {
  who: string;
  n: number;
}

export interface ArcadeBoardData {
  stubs: ArcadeBoardRow[];
  plays: ArcadeBoardRow[];
}

function BoardList({
  title,
  rows,
}: {
  title: string;
  rows: ArcadeBoardRow[];
}) {
  return (
    <div className="arc-board-col">
      <h4>{title}</h4>
      {rows.length === 0 ? (
        <div className="arc-note">—</div>
      ) : (
        <ol>
          {rows.map((r, i) => (
            <li key={r.who}>
              <span className="rk num">{i + 1}</span>
              <span className="who">{r.who}</span>
              <span className="nv num">{r.n}</span>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

export function ArcadeBoard({ board }: { board: ArcadeBoardData }) {
  const { dict } = useI18n();
  const t = dict.games.arcade;
  return (
    <section className="arc-sec">
      <h3>{t.boardTitle}</h3>
      <div className="arc-board">
        <BoardList title={t.boardStubs} rows={board.stubs} />
        <BoardList title={t.boardPlays} rows={board.plays} />
      </div>
      <p className="arc-note">{t.boardNote}</p>
    </section>
  );
}
