/**
 * 筹码面额栏：三枚快捷面额筹码（红 100 / 绿 1000 / 蓝 1万）。
 * 仅切换本注面额，不直接下注；超过单注上限的档位置灰不可选。
 */

import { useI18n } from "@/i18n/client";

interface Denom {
  value: number;
  tone: "red" | "green" | "blue";
}

const DENOMS: Denom[] = [
  { value: 100, tone: "red" },
  { value: 1000, tone: "green" },
  { value: 10000, tone: "blue" },
];

export function ChipDenoms({
  value,
  onChange,
  maxBet,
  disabled,
}: {
  value: number;
  onChange: (n: number) => void;
  maxBet: number;
  disabled?: boolean;
}) {
  const { dict } = useI18n();
  const tg = dict.games.bigsmall;
  const label = (v: number) =>
    v === 10000 ? tg.roomChip10k : String(v);
  return (
    <div className="bs-chips" role="group" aria-label={tg.roomChipsLabel}>
      {DENOMS.map((d) => {
        const off = disabled || d.value > maxBet;
        const active = value === d.value && !off;
        return (
          <button
            key={d.value}
            type="button"
            disabled={off}
            aria-pressed={active}
            onClick={() => onChange(d.value)}
            className={
              `bs-chip bs-chip--${d.tone}${active ? " is-active" : ""}`
            }
          >
            <span className="bs-chip-inner">{label(d.value)}</span>
          </button>
        );
      })}
    </div>
  );
}
