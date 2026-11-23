"use client";

import { useState } from "react";
import Link from "next/link";
import { useI18n } from "@/i18n/client";
import {
  useSfxToggle,
  useSfx,
} from "@/components/game/use-sfx";
import { BalanceBar } from "@/components/game/game-balance";

/* 余额条已拆到 game-balance.tsx（外壳行数门禁）；沿用原入口，
   农场等既有页面的 import 不用改。 */
export { BalanceBar } from "@/components/game/game-balance";

/**
 * 娱乐屋公共 UI 件（Aurora token 口径，组件内不硬编码色值）。
 * 四个玩法专注页共用：外壳 / 余额条 / 筹码 / 结果飘字 / 战绩条。
 * （结果飘字 / 浮层提示 / 战绩条拆到 game-kit-feedback.tsx）
 */

/** 音效开关（默认关口径）：偏好落 localStorage.arcade_sfx，全玩法共用一份。
 *  aria-label 走 dict，图标外显状态；开通瞬间那一声 tick 正是手势内解锁。 */
function SfxToggle({ showLabel = false }: { showLabel?: boolean }) {
  const { dict } = useI18n();
  const { on, toggle } = useSfxToggle();
  const state = on ? dict.games.sfx.on : dict.games.sfx.off;
  const label = `${dict.games.sfx.label}：${state}`;
  return (
    <button
      type="button"
      onClick={toggle}
      aria-pressed={on}
      aria-label={label}
      title={label}
      className={`ml-auto min-h-11 rounded-full border px-3 text-xs font-bold ${
        on
          ? "border-[var(--sky-deep)] bg-[var(--sky-soft)] " +
            "text-[var(--sky-deep)]"
          : "border-line text-sub"
      }`}
    >
      <span aria-hidden>{on ? "🔊" : "🔇"}</span>
      {showLabel && <span className="sfx-state">{state}</span>}
    </button>
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
  headerLayout = "default",
  badges,
  tabBar,
  balanceVariant = "inline",
  balanceLabel,
  limitLabel,
  stageBox = true,
  singleColumn = false,
}: {
  title: string;
  subtitle?: string;
  /** 标题前图标：线条 SVG（ArcadeGlyph）或 emoji 均可 */
  icon: React.ReactNode;
  balance: number | null;
  todayNet: number | null;
  limitLeft: number | null;
  /** 提示条（防沉迷等），渲染在余额条下方 */
  notice?: React.ReactNode;
  stage: React.ReactNode;
  controls?: React.ReactNode;
  side?: React.ReactNode;
  foot?: React.ReactNode;
  /** center：样图的大转盘头——返回键在左、标题+徽章居中、音效在右 */
  headerLayout?: "default" | "center";
  /** 标题下的徽章行（中奖率 / 票价） */
  badges?: React.ReactNode;
  /** 与余额同排的标签条（大转盘三标签） */
  tabBar?: React.ReactNode;
  balanceVariant?: "inline" | "chips";
  balanceLabel?: string;
  limitLabel?: string;
  /** false：舞台内容自带卡片（奖池/记录标签页），不再套一层舞台盒 */
  stageBox?: boolean;
  /** 单列：没有侧栏的专注页（大转盘三标签） */
  singleColumn?: boolean;
}) {
  const { dict } = useI18n();
  const back = (
    <Link
      href="/games"
      className="gs-back min-h-11 rounded-full border border-line px-3 text-xs font-bold text-sub"
    >
      ← {dict.games.back}
    </Link>
  );
  return (
    <div className="gs-shell flex flex-col gap-4">
      {headerLayout === "center" ? (
        <div className="gs-head-center">
          {back}
          <div className="gs-head-mid">
            <div className="gs-head-line">
              <h1 className="gs-title font-display text-2xl">
                <span aria-hidden className="gs-title-ic">
                  {icon}
                </span>{" "}
                {title}
              </h1>
              {badges && <div className="gs-head-badges">{badges}</div>}
            </div>
            {subtitle && (
              <span className="gs-head-sub">{subtitle}</span>
            )}
          </div>
          <SfxToggle showLabel />
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          {back}
          <h1 className="gs-title font-display text-2xl">
            <span aria-hidden className="gs-title-ic">
              {icon}
            </span>{" "}
            {title}
          </h1>
          {subtitle && <span className="text-sm text-sub">{subtitle}</span>}
          <SfxToggle />
        </div>
      )}
      {tabBar ? (
        <div className="gs-meta-row">
          <BalanceBar
            balance={balance}
            todayNet={todayNet}
            limitLeft={limitLeft}
            variant={balanceVariant}
            balanceLabel={balanceLabel}
            limitLabel={limitLabel}
          />
          {tabBar}
        </div>
      ) : (
        <BalanceBar
          balance={balance}
          todayNet={todayNet}
          limitLeft={limitLeft}
        />
      )}
      {notice}
      <div
        className={`gs-body grid grid-cols-1 gap-4${
          singleColumn ? "" : " lg:grid-cols-[minmax(0,1fr)_280px]"
        }`}
      >
        <div className="flex flex-col gap-4">
          {stageBox ? (
            <div className="gs-stage-box flex flex-col items-center gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-raised)] p-5 shadow-[var(--shadow-card)]">
              {stage}
            </div>
          ) : (
            stage
          )}
          {controls}
        </div>
        {side && <div className="gs-side-col flex flex-col gap-4">{side}</div>}
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
    <div className="flex flex-wrap items-center gap-2 rounded-[var(--r-md)] bg-[var(--sun-soft)] px-3 py-2 text-xs font-bold text-[var(--ink)]">
      <span aria-hidden>🧘</span>
      <span className="flex-1">{parts.join(" · ")}</span>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="min-h-11 rounded-full border border-[var(--border-deep)] px-3 text-xs font-bold"
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
  const { play } = useSfx();
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
          onClick={() => {
            play("tick");
            onChange(v);
          }}
          aria-pressed={value === v}
          className="arc-chip num"
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
        className="num min-h-11 w-24 rounded-full border border-line bg-[var(--surface-card)] px-3 text-right font-bold outline-none focus:ring-2 focus:ring-[var(--focus-ring)]"
      />
      <span className="text-xs text-sub">{currency}</span>
    </div>
  );
}
