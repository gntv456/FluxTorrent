"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";

/**
 * 娱乐屋公共 UI 件（Aurora token 口径，组件内不硬编码色值）。
 * 四个玩法专注页共用：外壳 / 余额条 / 筹码 / 结果飘字 / 战绩条。
 */

/** 余额条：余额 + 今日净收 + 剩余局数（下注前先看得见，防沉迷设计的一部分） */
export function BalanceBar({
  balance,
  todayNet,
  limitLeft,
}: {
  balance: number | null;
  todayNet: number | null;
  limitLeft: number | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-1 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-4 py-3 shadow-[var(--shadow-card)]">
      <span className="flex items-baseline gap-1.5">
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
                ? "text-mint"
                : "text-danger"
          }`}
        >
          {todayNet === null ? "—" : `${todayNet > 0 ? "+" : ""}${todayNet}`}
        </b>
      </span>
      <span className="flex items-baseline gap-1 text-xs text-sub">
        {t.limitLeft.replace("{n}", limitLeft === null ? "—" : String(limitLeft))}
      </span>
    </div>
  );
}

/** 专注页外壳：返回大厅 + 标题 + 舞台 + 控制区 + 侧栏 */
export function GameShell({
  title,
  subtitle,
  icon,
  balance,
  todayNet,
  limitLeft,
  stage,
  controls,
  side,
  foot,
}: {
  title: string;
  subtitle?: string;
  icon: string;
  balance: number | null;
  todayNet: number | null;
  limitLeft: number | null;
  stage: React.ReactNode;
  controls?: React.ReactNode;
  side?: React.ReactNode;
  foot?: React.ReactNode;
}) {
  const { dict } = useI18n();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Link
          href="/games"
          className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-sub"
        >
          ← {dict.games.back}
        </Link>
        <h1 className="font-display text-2xl">
          <span aria-hidden>{icon}</span> {title}
        </h1>
        {subtitle && <span className="text-sm text-sub">{subtitle}</span>}
      </div>
      <BalanceBar balance={balance} todayNet={todayNet} limitLeft={limitLeft} />
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_280px]">
        <div className="flex flex-col gap-4">
          <div className="flex flex-col items-center gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-raised)] p-5 shadow-[var(--shadow-card)]">
            {stage}
          </div>
          {controls}
        </div>
        {side && <div className="flex flex-col gap-4">{side}</div>}
      </div>
      {foot}
    </div>
  );
}

/** 筹码选择：预设档 + 自定义输入，受 maxBet 约束 */
export function ChipSelect({
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
  const { dict, currency } = useI18n();
  const t = dict.games;
  const presets = [10, 50, 100, 500, maxBet].filter(
    (v, i, a) => v <= maxBet && a.indexOf(v) === i,
  );
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-xs text-sub">{t.bet}</span>
      {presets.map((v) => (
        <button
          key={v}
          type="button"
          disabled={disabled}
          onClick={() => onChange(v)}
          aria-pressed={value === v}
          className={`num min-h-[40px] rounded-full px-3.5 text-sm font-bold ${
            value === v
              ? "bg-[image:var(--grad-aurora)] text-white"
              : "border border-[var(--border-deep)] bg-[var(--surface-card)] text-ink"
          } disabled:opacity-50`}
        >
          {v}
        </button>
      ))}
      <input
        type="number"
        min={1}
        max={maxBet}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Math.max(1, Math.min(maxBet, Number(e.target.value) || 1)))}
        aria-label={t.betLabel.replace("{magic}", currency)}
        className="num min-h-[40px] w-24 rounded-full border border-line bg-[var(--surface-card)] px-3 text-right font-bold outline-none focus:ring-2 focus:ring-sky/40"
      />
      <span className="text-xs text-sub">{currency}</span>
    </div>
  );
}

/** 结果飘字：赢/平/输/大奖 四态，颜色之外另有文案与图标（色觉无障碍） */
export function ResultFlash({
  kind,
  text,
}: {
  kind: "win" | "lose" | "tie" | "jackpot" | null;
  text: string | null;
}) {
  const cls =
    kind === "jackpot"
      ? "text-[var(--warning)]"
      : kind === "win"
        ? "text-mint"
        : kind === "lose"
          ? "text-sub"
          : "text-ink";
  return (
    <p
      role="status"
      aria-live="polite"
      className={`flex min-h-[36px] items-center justify-center gap-1.5 text-center text-base font-black ${cls}`}
    >
      {text && <span className="animate-[fly_.28s_ease-out]">{text}</span>}
    </p>
  );
}

/** 战绩条：来自 spark_ledger（/games/history），刷新不丢、跨设备一致 */
export function HistoryStrip({
  items,
  empty,
}: {
  items: { ref_type: string | null; amount: number }[];
  empty?: string;
}) {
  const { dict } = useI18n();
  if (items.length === 0) {
    return <p className="py-2 text-center text-xs text-sub">{empty ?? dict.games.historyEmpty}</p>;
  }
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {items.map((h, i) => {
        const win = h.amount > 0;
        const jack = h.amount >= 500;
        return (
          <span
            key={i}
            title={`${h.ref_type ?? "game"} ${h.amount > 0 ? "+" : ""}${h.amount}`}
            className={`inline-block h-4 w-4 rounded-full ${
              jack
                ? "bg-sun"
                : win
                  ? "bg-mint"
                  : "border border-[var(--border-deep)] bg-[var(--surface-sunken)]"
            }`}
          />
        );
      })}
    </div>
  );
}
