"use client";
import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { cardLabel, type DzGame, type Seat } from "./room-types";

/** 0..12 = 3..2；13 小王；14 大王（与 combo/rank 口径一致）。 */
const rankOf = (c: number) => (c >= 52 ? 13 + (c - 52) : Math.floor(c / 4));

/** 各点数初始张数：普通点 4 张，双王各 1 张。 */
const INITIAL = [
  4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 1, 1,
];

/**
 * 记牌器：本机推算“外面还剩”的各点数张数 = 初始 − 自己手牌 − 已打出。
 * 只做展示，不上传服务器；癞子按实牌统计，不在前端展开万能替代。
 */
export function DdCounter({ game, me }: { game: DzGame; me?: Seat }) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const [open, setOpen] = useState(false);

  const played = new Array<number>(INITIAL.length).fill(0);
  for (const e of game.events ?? game.events_tail ?? []) {
    if (e.action !== "play") continue;
    for (const c of e.cards ?? []) played[rankOf(c)] += 1;
  }
  const mine = new Array<number>(INITIAL.length).fill(0);
  for (const c of me?.cards ?? []) mine[rankOf(c)] += 1;

  const remaining = INITIAL.map(
    (init, r) => Math.max(0, init - mine[r] - played[r]),
  );
  const total = remaining.reduce((s, n) => s + n, 0);

  const label = (r: number) =>
    r === 13 ? "小王" : r === 14 ? "大王" : cardLabel(r * 4);

  return (
    <div className="dz-counter">
      <button
        type="button"
        className="dz-btn dz-counter-btn"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        {t.counterBtn}
        {open && total > 0 && (
          <span className="dz-counter-total">{total}</span>
        )}
      </button>
      {open && (
        <div className="dz-counter-panel" role="group" aria-label={t.counterTitle}>
          <div className="dz-counter-grid">
            {remaining.map((count, r) => {
              const cls = [
                "dz-counter-cell",
                count <= 0 ? "is-zero" : "",
                count >= 4 ? "is-full" : "",
                r >= 13 ? "is-joker" : "",
              ]
                .filter(Boolean)
                .join(" ");
              return (
                <div key={r} className={cls}>
                  <b>{label(r)}</b>
                  <span>{count > 0 ? count : t.counterNone}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
