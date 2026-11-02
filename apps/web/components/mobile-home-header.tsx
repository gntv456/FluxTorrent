"use client";

import { useI18n } from "@/i18n/client";

/** 首页移动仪表盘（M3 方案 §5 + 11-02 重设计）：<md 显示。
 *
 *  原来只是一行问候语 + 四个白盒子（上传/下载/分享率/魔力），信息量与
 *  层级都撑不起首屏；现在升级成仪表盘：
 *    问候 + 签到概况 / 魔力余额（主数字）/ 上传·下载·分享率 /
 *    分享率处置线刻度条（短于刻度＝低于处置线，珊瑚色告警）。
 *
 *  数据来自 /me/overview（个人口径，由调用方 RSC 注入）+ /home 的
 *  ratio_watch_threshold（站点处置线）；组件只做展示，不自己取数。 */

/** 仪表盘要用的个人四项（/me/overview 摘要的形状）。 */
export interface HomeMeView {
  uploaded: string;
  downloaded: string;
  ratio: string;
  spark: string;
  ratioValue: number | null;
}

function greet(h: number, t: { m: string; a: string; e: string; n: string }) {
  if (h < 6) return t.n;
  if (h < 12) return t.m;
  if (h < 18) return t.a;
  return t.e;
}

/** 站点时区（UTC+8）下的当前小时。
 *
 *  ZT2（2026-10-03）修水合不匹配：此前直接 `new Date().getHours()` —— web 容器跑
 *  UTC（本地 01:07 时容器是 17:07 → 渲染「下午好」），浏览器跑 UTC+8（渲染「夜深了」），
 *  SSR 与首次客户端渲染文本不一致 → React 每次进首页报 #418（text mismatch）并在
 *  首屏闪一下。改成两端都按**站点时区**取小时，结果一致，且对站内用户更符合预期。
 *  取不到（ICU 异常）时返回 -1，由调用方回落到中性问候，绝不渲染 NaN。 */
function siteHour(): number {
  try {
    const s = new Intl.DateTimeFormat("en-GB", {
      timeZone: "Asia/Shanghai",
      hour: "2-digit",
      hour12: false,
    }).format(new Date());
    const h = Number(s) % 24;
    return Number.isFinite(h) ? h : -1;
  } catch {
    return -1;
  }
}

/** 分享率处置线刻度条。
 *
 *  刻度按「处置线的 1.5 倍」铺满，处置线本身落在 2/3 处——用户的柱子短于
 *  刻度线就是低于处置线。阈值取不到（后端没下发）时整条不画：画一条没有
 *  刻度的进度条等于骗人说「你离处置线还有一截」。 */
function RatioGauge({
  ratio,
  threshold,
  lowLabel,
  okLabel,
}: {
  ratio: number | null;
  threshold: number | null;
  lowLabel: string;
  okLabel: string;
}) {
  if (ratio === null || threshold === null || threshold <= 0) return null;
  const scale = threshold * 1.5;
  const pct = Math.min(100, (ratio / scale) * 100);
  const mark = (threshold / scale) * 100;
  const low = ratio < threshold;
  return (
    <div className="mhome-gauge">
      <div className="mhome-gauge__bar">
        <i className={low ? "is-low" : ""} style={{ width: `${pct}%` }} />
        <u style={{ left: `${mark}%` }} />
      </div>
      <p className={low ? "mhome-gauge__note is-low" : "mhome-gauge__note"}>
        {low ? lowLabel : okLabel}
      </p>
    </div>
  );
}

export function MobileHomeHeader({
  brand,
  me,
  streak,
  totalDays,
  threshold,
}: {
  brand: string;
  me: HomeMeView | null;
  /** 连签 / 累计签到（来自 /home 的 attendance 段；模块关闭时为 null） */
  streak?: number | null;
  totalDays?: number | null;
  /** 站点分享率处置线（ratio_watch_threshold） */
  threshold?: number | null;
}) {
  const { dict } = useI18n();
  const t = dict.mhome;
  const h = siteHour();
  // -1（ICU 异常）→ 中性问候，避免 SSR/CSR 分歧又避免出现 NaN
  const hello = h < 0 ? t.a : greet(h, t);
  const hasStreak = typeof streak === "number" && typeof totalDays === "number";
  const fmt = (v: number) => v.toFixed(2);
  const gap = threshold != null && me?.ratioValue != null
    ? Math.max(0, threshold - me.ratioValue).toFixed(2)
    : "0";
  return (
    <div className="mhome-hero md:hidden">
      <div className="mhome-hero__row">
        <span className="mhome-hero__lg" aria-hidden="true">
          FT
        </span>
        <span className="mhome-hero__hi">
          <b>
            {hello}，{brand}
          </b>
          {hasStreak ? (
            <i>
              {t.streakLine
                .replace("{streak}", String(streak))
                .replace("{total}", String(totalDays))}
            </i>
          ) : null}
        </span>
        {me ? (
          <dl className="mhome-hero__spark num">
            <div>
              <dt>{t.spark}</dt>
              <dd>★ {me.spark}</dd>
            </div>
          </dl>
        ) : null}
      </div>
      {me ? (
        <>
          <dl className="mhome-hero__stats num">
            <div>
              <dt>{t.up}</dt>
              <dd>↑ {me.uploaded}</dd>
            </div>
            <div>
              <dt>{t.down}</dt>
              <dd>↓ {me.downloaded}</dd>
            </div>
            <div>
              <dt>{t.ratio}</dt>
              <dd>∑ {me.ratio}</dd>
            </div>
          </dl>
          <RatioGauge
            ratio={me.ratioValue}
            threshold={threshold ?? null}
            lowLabel={t.gaugeLow
              .replace("{ratio}", fmt(me.ratioValue ?? 0))
              .replace("{threshold}", fmt(threshold ?? 0))
              .replace("{gap}", gap)}
            okLabel={t.gaugeOk
              .replace("{ratio}", fmt(me.ratioValue ?? 0))
              .replace("{threshold}", fmt(threshold ?? 0))}
          />
        </>
      ) : null}
    </div>
  );
}
