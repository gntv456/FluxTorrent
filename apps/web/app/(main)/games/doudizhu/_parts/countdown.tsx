"use client";
import { useEffect, useRef, useState } from "react";
import { useI18n } from "@/i18n/client";
import { playSfx } from "@/lib/sfx";
import type { DzGame } from "./room-types";

/** 出牌/叫分倒计时条：本地递减，归零触发 onExpire（代发托管动作）。
 *  turn 变化即重置；≤5s 变红示警，宽度随剩余时间收缩。
 *  每个 turnKey 只 fire 一次：idle（归零未轮转）期间 onExpire 引用随
 *  父组件渲染重建，useEffect 依赖一变就重触发，代打会连打（hint 独立
 *  管线绕过 busy 闸，实测倒计时归零后一手接一手把牌快速走完）——
 *  firedRef 按轮去重，轮转/重置（turnKey 变）自动解锁。 */
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
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const [left, setLeft] = useState(seconds);
  const firedRef = useRef<string | null>(null);
  useEffect(() => {
    setLeft(seconds);
    if (game.my_turn) playSfx("turn");
    const tick = window.setInterval(
      () => setLeft((c) => Math.max(0, c - 1)),
      1000,
    );
    return () => window.clearInterval(tick);
  }, [turnKey, seconds, game.my_turn]);
  useEffect(() => {
    if (game.my_turn && left >= 1 && left <= 5) playSfx("warn");
  }, [left, game.my_turn]);
  const idle = game.my_turn && left <= 0;
  useEffect(() => {
    if (!idle) return;
    if (firedRef.current === turnKey) return;
    firedRef.current = turnKey;
    onExpire();
  }, [idle, turnKey, onExpire]);
  if (!game.my_turn) return null;
  const pct = Math.max(
    0,
    Math.min(100, Math.round((left / Math.max(1, seconds)) * 100)),
  );
  return (
    <div
      className={`dz-countdown${left <= 5 ? " is-low" : ""}`}
      role="timer"
      aria-label={t.countdownLabel}
    >
      <span className="dz-countdown-you">{t.countdownYou}</span>
      <span className="dz-countdown-fill" style={{ width: `${pct}%` }} />
      <span className="dz-countdown-num">{left}s</span>
    </div>
  );
}
