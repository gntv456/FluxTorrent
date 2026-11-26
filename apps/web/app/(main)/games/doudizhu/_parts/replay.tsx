"use client";
import { useI18n } from "@/i18n/client";
import { cardLabel, type DzEvent } from "./room-types";

/** 单条事件 → 可读行（叫分/出牌/过）。 */
function evLine(
  t: ReturnType<typeof useI18n>["dict"]["games"]["doudizhu"],
  e: DzEvent,
  mySeat: number,
): string {
  const who = e.seat === 0 || e.seat === mySeat ? t.you : "P" + (e.seat + 1);
  if (e.action === "bid") {
    const pts =
      e.score && e.score > 0
        ? t.scoreUnit.replace("{n}", String(e.score))
        : t.bid0;
    return `${who} · ${pts}`;
  }
  if (e.action === "pass") {
    return `${who} · ${t.passBtn}`;
  }
  return `${who} · ${(e.cards ?? []).map(cardLabel).join(" ")}`;
}

/** 结算回放面板：完整出牌流水（events 全量），行数有限直接铺。 */
export function ReplayPanel({
  events,
  mySeat,
}: {
  events: DzEvent[];
  mySeat: number;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  if (!events.length) return null;
  return (
    <details className="dz-replay">
      <summary>{t.replayTitle}</summary>
      <ol className="dz-replay-list">
        {events.map((e, i) => (
          <li
            key={i}
            className={e.seat === mySeat ? "is-me" : undefined}
          >
            {evLine(t, e, mySeat)}
          </li>
        ))}
      </ol>
    </details>
  );
}
