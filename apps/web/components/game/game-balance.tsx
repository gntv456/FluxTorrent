"use client";

import { useI18n } from "@/i18n/client";

/**
 * 余额条：余额 + 今日净收 + 剩余次数（下注前先看得见，
 * 防沉迷设计的一部分）。
 * `limitText` 覆盖默认文案 —— 农场用的是自己的限流配额（rl:farm）；
 * `chips` 是样图的三宝石卡形态（余额/今日净收/剩余局数各占一格）。
 */
export function BalanceBar({
  balance,
  todayNet,
  limitLeft,
  limitText,
  variant = "inline",
  balanceLabel,
  limitLabel,
}: {
  balance: number | null;
  todayNet: number | null;
  limitLeft: number | null;
  limitText?: string;
  variant?: "inline" | "chips";
  balanceLabel?: string;
  limitLabel?: string;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const netText =
    todayNet === null ? "—" : `${todayNet > 0 ? "+" : ""}${todayNet}`;
  if (variant === "chips") {
    const chips = [
      {
        ic: "💎",
        lb: balanceLabel ?? currency,
        vl: balance === null ? "—" : balance.toLocaleString(),
        cls: "",
      },
      {
        ic: "💰",
        lb: t.todayNet,
        vl: netText,
        cls:
          todayNet === null || todayNet === 0
            ? ""
            : todayNet > 0
              ? "win"
              : "lose",
      },
      {
        ic: "🎯",
        lb: limitLabel ?? t.limitLeft.replace("{n}", ""),
        vl: limitLeft === null ? "—" : String(limitLeft),
        cls: "",
      },
    ];
    return (
      <div className="wp-bal-row">
        {chips.map((c) => (
          <span key={c.lb} className="wp-bal-chip">
            <span className="wp-bal-ic" aria-hidden>
              {c.ic}
            </span>
            <span className="wp-bal-mid">
              <span className="wp-bal-lb">{c.lb}</span>
              <span className={`wp-bal-vl num ${c.cls}`.trimEnd()}>
                {c.vl}
              </span>
            </span>
          </span>
        ))}
      </div>
    );
  }
  return (
    <div
      className={
        "gs-balance flex flex-wrap items-center gap-x-5 gap-y-1 " +
        "rounded-[var(--r-md)] border border-line " +
        "bg-[var(--surface-card)] px-4 py-3 " +
        "shadow-[var(--shadow-card)]"
      }
    >
      <span className="flex items-baseline gap-1.5" aria-live="polite">
        <span aria-hidden>💎</span>
        <span className="num text-base font-black">
          {balance === null ? "—" : balance.toLocaleString()}
        </span>
        <span className="text-xs text-sub">{currency}</span>
      </span>
      <span className="flex items-baseline gap-1 text-xs text-sub">
        {t.todayNet}
        <b
          className={`num text-sm ${
            todayNet === null || todayNet === 0
              ? "text-sub"
              : todayNet > 0
                ? "text-[var(--mint)]"
                : "text-[var(--danger-text)]"
          }`}
        >
          {netText}
        </b>
      </span>
      <span className="flex items-baseline gap-1 text-xs text-sub">
        {limitText ??
          t.limitLeft.replace(
            "{n}",
            limitLeft === null ? "—" : String(limitLeft),
          )}
      </span>
    </div>
  );
}
