"use client";

import { fmtMult } from "@/lib/games";
import { useI18n } from "@/i18n/client";

/** 可挂载的猜大小道具（后端 /games 的 props 下发） */
export interface PropView {
  key: string;
  name: string;
  icon: string;
  /** mult 倍率 | shield 护盾 */
  effect: string;
  /** 千分值：mult=倍率、shield=返还比例 */
  value: number;
  held: number;
}

/**
 * 猜大小道具栏：道具**只加权魔力的输赢，永不出物品**。
 * 只负责展示与选择；同类效果互斥（真正只收一件的是服务端），
 * 所以这里不做叠乘，也不发请求。
 */
export function PropBar({
  props,
  sel,
  busy,
  onToggle,
}: {
  props: PropView[];
  sel: string[];
  busy: boolean;
  onToggle: (p: PropView) => void;
}) {
  const { dict } = useI18n();
  const tg = dict.games.bigsmall;
  const text = (p: PropView) =>
    p.effect === "mult"
      ? tg.propMult.replace("{n}", fmtMult(p.value / 1000))
      : tg.propShield.replace("{n}", String(p.value / 10));

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-xs text-sub">{tg.propsLabel}</span>
      {props.length === 0 ? (
        <span className="text-[11px] text-sub">{tg.propEmpty}</span>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {props.map((p) => (
            <button
              key={p.key}
              type="button"
              disabled={busy}
              aria-pressed={sel.includes(p.key)}
              onClick={() => onToggle(p)}
              className="arc-chip text-xs"
            >
              <span aria-hidden>{p.icon}</span> {text(p)}
              <span className="num"> ×{p.held}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
