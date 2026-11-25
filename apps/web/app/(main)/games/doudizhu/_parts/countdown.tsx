"use client";
import { useEffect, useState } from "react";
import type { DzGame } from "./room-types";

/** 出牌/叫分倒计时条：本地递减，归零触发 onExpire（代发托管动作）。
 *  turn 变化即重置；≤5s 变红示警，宽度随剩余时间收缩。 */
export function CountdownBar({
  game,
  seconds,
  onExpire,
}: {
  game: DzGame;
  seconds: number;
  onExpire: () => void;
}) {
  const turnKey = `${game.id}:${game.phase}:${game.turn}`;
  const [left, setLeft] = useState(seconds);
  useEffect(() => {
    setLeft(seconds);
    const tick = window.setInterval(
      () => setLeft((c) => Math.max(0, c - 1)),
      1000,
    );
    return () => window.clearInterval(tick);
  }, [turnKey, seconds]);
  const idle = game.my_turn && left <= 0;
  useEffect(() => {
    if (idle) onExpire();
  }, [idle, onExpire]);
  if (!game.my_turn) return null;
  const pct = Math.max(
    0,
    Math.min(100, Math.round((left / Math.max(1, seconds)) * 100)),
  );
  return (
    <div
      className={`dz-countdown${left <= 5 ? " is-low" : ""}`}
      role="timer"
      aria-label="countdown"
    >
      <span className="dz-countdown-fill" style={{ width: `${pct}%` }} />
      <span className="dz-countdown-num">{left}s</span>
    </div>
  );
}
