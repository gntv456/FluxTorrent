"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { uxMark } from "@/lib/ux-metrics";
import type { HomeData } from "@/components/home-data";
import type { HomeMeView } from "@/components/mobile-home-header";
import { attOf } from "@/components/home-attendance-shared";

/** 驾驶舱比移动仪表盘多读两个已取到的 /me/overview 字段（不新增请求）：
 *  username 用于问候语，seeding 用于「正在做种」；hr 缺省时以处置线做风险代理。 */
export interface CockpitMe extends HomeMeView {
  username?: string;
  seeding?: number;
  hr?: number;
}

function Stat({
  label,
  value,
  unit,
  watch,
  safeLabel,
  watchLabel,
}: {
  label: string;
  value: string;
  unit?: string;
  watch: boolean;
  safeLabel: string;
  watchLabel: string;
}) {
  return (
    <div className="ux-cockpit__stat">
      <dt>{label}</dt>
      <dd className="num">
        {value}
        {unit ? <small>{unit}</small> : null}
      </dd>
      <span className={watch ? "ux-chip is-watch" : "ux-chip is-safe"}>
        {watch ? watchLabel : safeLabel}
      </span>
    </div>
  );
}

/** 我的 PT 驾驶舱：排版板块之前的个人健康概览 + 三个主动作。
 *  纯展示组件：数据全部由调用方（HomeSections 已有的 props）注入，不取数。
 *  SSR 安全——无 window/localStorage 直读，时间口径也不在这里取。 */
export function HomeCockpit({
  me,
  home,
  threshold,
  currency,
  brand,
  checkin,
  checkinBusy,
  checkinMsg,
  onOpenCalendar,
  onReopenOnboarding,
}: {
  me: CockpitMe | null;
  home: HomeData;
  threshold?: number | null;
  currency: string;
  brand: string;
  checkin: () => Promise<void>;
  checkinBusy: boolean;
  checkinMsg: string | null;
  onOpenCalendar: () => void;
  onReopenOnboarding: () => void;
}) {
  const { dict } = useI18n();
  const u = dict.uxOpt;
  // 未登录/概览取不到时整块不渲染（新手引导同样只在 me 非空时出现）
  if (!me) return null;

  const att = attOf(home);
  const lowRatio =
    me.ratioValue != null &&
    threshold != null &&
    threshold > 0 &&
    me.ratioValue < threshold;
  // 个人 H&R 由 /me/overview 的 hr_unresolved 下发；老后端缺字段时降级
  // 为处置线代理语义，数值位留 —，绝不把站点级 warned 数冒充个人 H&R。
  const hrWatch = me.hr != null ? me.hr > 0 : lowRatio;
  const seedWatch = me.seeding != null && me.seeding <= 0;

  return (
    <section className="ux-cockpit" aria-labelledby="ux-cockpit-title">
      <header className="ux-cockpit__head">
        <div>
          <p className="ux-cockpit__hi">
            {u.greeting.replace("{name}", me.username || brand)}
          </p>
          <h2 id="ux-cockpit-title" className="ux-cockpit__title">
            {u.cockpitTitle}
          </h2>
          <p className="ux-cockpit__sub">{u.cockpitHealth}</p>
        </div>
        <Link
          href="/help#survival"
          className="ux-cockpit__rulelink"
          data-ux-event="guide_open"
          data-ux-label="cockpit-rule"
        >
          {u.guideRuleLink}
        </Link>
        <button
          type="button"
          className="ux-cockpit__guide"
          data-ux-event="guide_open"
          data-ux-label="cockpit"
          onClick={onReopenOnboarding}
        >
          {u.onboardingTitle}
        </button>
      </header>

      <dl className="ux-cockpit__stats">
        <Stat
          label={u.statRatio}
          value={me.ratio}
          watch={lowRatio}
          safeLabel={u.stateSafe}
          watchLabel={u.stateWatch}
        />
        <Stat
          label={u.statHr}
          value={me.hr != null ? String(me.hr) : "—"}
          watch={hrWatch}
          safeLabel={u.stateSafe}
          watchLabel={u.stateWatch}
        />
        <Stat
          label={u.statSeeding}
          value={me.seeding != null ? String(me.seeding) : "—"}
          watch={seedWatch}
          safeLabel={u.stateSafe}
          watchLabel={u.stateWatch}
        />
        <Stat
          label={u.statMagic}
          value={me.spark}
          unit={currency}
          watch={false}
          safeLabel={u.stateSafe}
          watchLabel={u.stateWatch}
        />
      </dl>

      {att ? (
        <p className="ux-cockpit__att">
          {u.streak.replace("{n}", String(att.streak))}
          {!att.checked_today && att.first_reward
            ? " · " + u.checkinReward.replace("{n}", String(att.first_reward))
            : ""}
        </p>
      ) : null}
      {checkinMsg ? (
        <p className="ux-cockpit__msg" role="status">
          {checkinMsg}
        </p>
      ) : null}

      <div className="ux-cockpit__actions">
        <Link
          href="/torrents"
          className="ux-btn ux-btn--primary"
          onClick={() => uxMark("first_torrent_open", "cockpit-search")}
        >
          {u.actionSearch}
        </Link>
        <Link
          href="/upload"
          className="ux-btn ux-btn--ghost"
          data-ux-event="publish_start"
          data-ux-label="cockpit"
          onClick={() => uxMark("first_torrent_open", "cockpit-publish")}
        >
          {u.actionPublish}
        </Link>
        {att ? (
          att.checked_today ? (
            <span className="ux-btn ux-btn--done" aria-live="polite">
              {u.checkinDone}
            </span>
          ) : (
            <button
              type="button"
              className="ux-btn ux-btn--checkin"
              data-ux-event="checkin"
              data-ux-label="cockpit"
              onClick={checkin}
              disabled={checkinBusy}
            >
              {checkinBusy ? dict.my.checkinBusy : u.actionCheckin}
              {att.first_reward ? (
                <i className="ux-btn__gain">+{att.first_reward}</i>
              ) : null}
            </button>
          )
        ) : (
          <button
            type="button"
            className="ux-btn ux-btn--ghost"
            onClick={onOpenCalendar}
          >
            {u.viewCalendar}
          </button>
        )}
      </div>
    </section>
  );
}
