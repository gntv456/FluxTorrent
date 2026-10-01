"use client";

import Link from "next/link";

/**
 * 样图 v2 富化模块三件：奖池条（prize-strip 金框缎面横排）/
 * 规则条（rules-bar 浅蓝一行）/ 农场天气条。文案由页面 i18n 合成传入。
 */

export interface SwPrizeItem {
  /** 图标（emoji 或首字符） */
  ic: string;
  nm: string;
  odds: string;
  /** 彩块色调：sky | gold | lilac | mint */
  tone?: "sky" | "gold" | "lilac" | "mint";
}

const TONES: Record<string, string> = {
  sky: "linear-gradient(135deg,#6a9ec8,#4a7ab0)",
  gold: "linear-gradient(135deg,#e8c878,#c89848)",
  lilac: "linear-gradient(135deg,#8898d8,#5a6ab8)",
  mint: "linear-gradient(135deg,#78b898,#4a8868)",
};

export function SwPrizeStrip({
  label,
  items,
}: {
  label: string;
  items: SwPrizeItem[];
}) {
  if (items.length === 0) return null;
  return (
    <div className="sw-prize-strip">
      <div className="sw-prize-lb">{label}</div>
      <div className="sw-prize-row">
        {items.map((it) => (
          <div key={it.nm} className="sw-prize-item">
            <div
              className="sw-prize-ic"
              style={
                { "--tone": TONES[it.tone ?? "sky"] } as React.CSSProperties
              }
            >
              <span aria-hidden>{it.ic}</span>
            </div>
            <div className="sw-prize-nm">{it.nm}</div>
            <div className="sw-prize-odds num">{it.odds}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function SwRules({
  left,
  linkLabel,
  linkHref,
}: {
  /** 左侧一句规则/口径说明（可含 <b> 强调由页面拼好纯文本） */
  left: string;
  /** 右侧「图鉴 ›」类链接；不传则只渲染左文案 */
  linkLabel?: string;
  linkHref?: string;
}) {
  return (
    <div className="sw-rules">
      <span className="sw-rules-t">{left}</span>
      {linkLabel && linkHref && (
        <Link href={linkHref}>
          {linkLabel}
        </Link>
      )}
    </div>
  );
}

export function SwFarmWeather({
  icon,
  main,
  right,
}: {
  icon: string;
  /** 「晴 · 今日浇水 +20% 生长速度」类主文案（页面拼好） */
  main: string;
  /** 右侧「4h 后刷新价格」类辅助文案 */
  right?: string;
}) {
  return (
    <div className="sw-farm-weather">
      <span aria-hidden>{icon}</span>
      <span>{main}</span>
      {right && <span className="wx-right">{right}</span>}
    </div>
  );
}
