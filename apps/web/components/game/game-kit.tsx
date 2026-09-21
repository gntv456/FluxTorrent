"use client";

import { useState } from "react";
import Link from "next/link";
import { useI18n } from "@/i18n/client";

/**
 * 娱乐屋公共 UI 件（Aurora token 口径，组件内不硬编码色值）。
 * 四个玩法专注页共用：外壳 / 余额条 / 筹码 / 结果飘字 / 战绩条。
 * （结果飘字 / 浮层提示 / 战绩条拆到 game-kit-feedback.tsx）
 */

/** 余额条：余额 + 今日净收 + 剩余次数（下注前先看得见，防沉迷设计的一部分）。
 *  `limitText` 覆盖默认文案 —— 农场用的是它自己的限流配额（rl:farm），口径不同。 */
export function BalanceBar({
  balance,
  todayNet,
  limitLeft,
  limitText,
}: {
  balance: number | null;
  todayNet: number | null;
  limitLeft: number | null;
  limitText?: string;
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
        {limitText ??
          t.limitLeft.replace(
            "{n}",
            limitLeft === null ? "—" : String(limitLeft),
          )}
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
  notice,
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
  /** 提示条（防沉迷等），渲染在余额条下方 */
  notice?: React.ReactNode;
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
      {notice}
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

/** 防沉迷软提示（策划案 §8.3）：连打多局 / 今日净输偏大时给一句提醒，可关掉，**不阻断**。 */
export function PlayHint({
  sessionPlays,
  todayNet,
}: {
  /** 本次会话已完成的局数 */
  sessionPlays: number;
  todayNet: number | null;
}) {
  const { dict } = useI18n();
  const t = dict.games;
  const [dismissed, setDismissed] = useState(false);
  const many = sessionPlays >= 20;
  const losing = todayNet !== null && todayNet <= -5000;
  if (dismissed || (!many && !losing)) return null;
  const parts = [
    many ? t.hintMany.replace("{n}", String(sessionPlays)) : null,
    losing
      ? t.hintLose
          .replace("{n}", String(-(todayNet ?? 0)))
          .replace("{magic}", "")
      : null,
  ].filter(Boolean);
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-[var(--r-md)] bg-sun-soft px-3 py-2 text-xs font-bold text-ink">
      <span aria-hidden>🧘</span>
      <span className="flex-1">{parts.join(" · ")}</span>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="min-h-[32px] rounded-full border border-[var(--border-deep)] px-3 text-[11px] font-bold"
      >
        {t.hintOk}
      </button>
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
        onChange={(e) =>
          onChange(Math.max(1, Math.min(maxBet, Number(e.target.value) || 1)))
        }
        aria-label={t.betLabel.replace("{magic}", currency)}
        className="num min-h-[40px] w-24 rounded-full border border-line bg-[var(--surface-card)] px-3 text-right font-bold outline-none focus:ring-2 focus:ring-sky/40"
      />
      <span className="text-xs text-sub">{currency}</span>
    </div>
  );
}
