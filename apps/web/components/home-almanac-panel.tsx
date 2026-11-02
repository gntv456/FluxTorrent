"use client";

import { useMemo } from "react";
import { useI18n } from "@/i18n/client";
import {
  almanacDetail,
  almanacMark,
  dayOfYear,
  isoWeek,
  type AlmanacDetail,
} from "@/components/home-almanac";

/** 签到日历的万年历形态（月历网格 + 选中日明细）。
 *
 *  每格三行：节气/节日标记（红=节气、绿=节日）、阳历日、农历日+日干支。
 *  历法数据由 lunar-javascript 现算（见 home-almanac.ts 的口径注释），
 *  签到状态只对当前月有效（/api/v1/attendance 只回近 30 天）。 */

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

export function ymd(y: number, m: number, d: number): string {
  return `${y}-${pad2(m)}-${pad2(d)}`;
}

export function AlmanacCalendar({
  year,
  month,
  selected,
  today,
  doneMap,
  onMonth,
  onPick,
}: {
  year: number;
  month: number;
  /** 选中日（YYYY-MM-DD） */
  selected: string;
  /** 站点今天（YYYY-MM-DD） */
  today: string;
  /** 已签到：日期 → 奖励数 */
  doneMap: Record<string, number>;
  onMonth: (y: number, m: number) => void;
  onPick: (date: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.alm;
  const cells = useMemo(() => {
    const dim = new Date(Date.UTC(year, month, 0)).getUTCDate();
    const lead = (new Date(Date.UTC(year, month - 1, 1)).getUTCDay() + 6) % 7;
    const out: ({ d: number; key: string } | null)[] = [];
    for (let i = 0; i < lead; i++) out.push(null);
    for (let d = 1; d <= dim; d++) {
      out.push({ d, key: ymd(year, month, d) });
    }
    return out;
  }, [year, month]);
  const prev = () => {
    if (month === 1) onMonth(year - 1, 12);
    else onMonth(year, month - 1);
  };
  const next = () => {
    if (month === 12) onMonth(year + 1, 1);
    else onMonth(year, month + 1);
  };
  return (
    <div className="alm-cal">
      <div className="alm-cal__bar">
        <button type="button" className="alm-cal__mv" onClick={prev}>
          ‹
        </button>
        <span className="alm-cal__title">
          {t.calTitle
            .replace("{y}", String(year))
            .replace("{m}", String(month))}
        </span>
        <button type="button" className="alm-cal__mv" onClick={next}>
          ›
        </button>
      </div>
      <div className="alm-cal__wd">
        {t.weekHead.map((w, i) => (
          <span key={w} className={i >= 5 ? "is-we" : ""}>
            {w}
          </span>
        ))}
      </div>
      <div className="alm-cal__grid">
        {cells.map((c, i) => {
          if (!c) return <i key={`b${i}`} className="alm-cal__out" />;
          const mk = almanacMark(year, month, c.d);
          const done = doneMap[c.key] != null;
          const cls = ["alm-cal__cell"];
          if (c.key === selected) cls.push("is-sel");
          if (c.key === today) cls.push("is-today");
          if (done) cls.push("is-done");
          return (
            <button
              type="button"
              key={c.key}
              className={cls.join(" ")}
              onClick={() => onPick(c.key)}
              aria-label={`${year}-${month}-${c.d}`}
            >
              <em className={mk?.isTerm ? "is-term" : ""}>
                {mk?.mark ?? ""}
              </em>
              <b>{c.d}</b>
              <s>
                {mk?.lunarDay ?? ""}
                {mk?.ganzhi ?? ""}
              </s>
            </button>
          );
        })}
      </div>
      <div className="alm-cal__legend">
        <span>
          <i className="is-done" />
          {t.signed}
        </span>
        <span>
          <i className="is-today" />
          {t.today}
        </span>
        <span>
          <i className="is-term" />
          {t.termOrFest}
        </span>
      </div>
    </div>
  );
}

/** 选中日的万年历明细（农历 / 干支 / 纳音 / 冲煞 / 值神 / 宜忌 / 建除 / 彭祖） */
export function AlmanacDetailPanel({ date }: { date: string }) {
  const { dict } = useI18n();
  const t = dict.alm;
  const [y, m, d] = date.split("-").map((x) => Number(x));
  const det: AlmanacDetail | null =
    y && m && d ? almanacDetail(y, m, d) : null;
  if (!det) {
    return <p className="alm-detail__empty">{t.outOfRange}</p>;
  }
  return (
    <div className="alm-detail">
      <p className="alm-detail__solar">
        {t.solarFmt
          .replace("{y}", String(det.y))
          .replace("{m}", String(det.m))
          .replace("{d}", String(det.d))
          .replace("{wd}", t.weekdays[det.weekday] ?? "")}{" "}
        · {t.dayOfYear.replace("{n}", String(dayOfYear(y, m, d)))}{" "}
        {t.weekOfYear.replace("{n}", String(isoWeek(y, m, d)))}
      </p>
      <p className="alm-detail__lunar">
        {t.lunarFmt
          .replace("{gz}", det.lunarYearGz)
          .replace("{animal}", det.lunarAnimal)
          .replace("{month}", det.lunarMonth)
          .replace("{day}", det.lunarDay)}
      </p>
      <p className="alm-detail__gz">
        {det.ganzhi} · {det.xun}
        {t.xunSuffix}
        {det.kong}
        {t.kongSuffix} · {det.zodiac}
      </p>
      <dl className="alm-detail__tri">
        <div>
          <dt>{t.nayin}</dt>
          <dd>{det.nayin}</dd>
        </div>
        <div>
          <dt>{t.chong}</dt>
          <dd>
            {t.chongWord}
            {det.chong}
            <s>
              {t.shaWord}
              {det.sha}
            </s>
          </dd>
        </div>
        <div>
          <dt>{t.tianShen}</dt>
          <dd>
            {det.tianShen}
            <s>
              {det.tianShenType}
              {det.luck}
            </s>
          </dd>
        </div>
      </dl>
      <div className="alm-detail__yj">
        <b>{t.yi}</b>
        <span>
          {det.yi.map((x) => (
            <s key={x}>{x}</s>
          ))}
        </span>
      </div>
      <div className="alm-detail__yj is-ji">
        <b>{t.ji}</b>
        <span>
          {det.ji.map((x) => (
            <s key={x}>{x}</s>
          ))}
        </span>
      </div>
      <div className="alm-detail__jc">
        {t.jianchu.map((x) => (
          <s key={x} className={x === det.zhiXing ? "is-on" : ""}>
            {x}
          </s>
        ))}
      </div>
      <p className="alm-detail__pz">
        {t.pengzu}
        {det.pengZuGan}；{det.pengZuZhi}
      </p>
    </div>
  );
}

/** 月历 + 明细的组合体：给 next/dynamic 懒加载用。
 *
 *  lunar-javascript 的表有 300KB；静态 import 会把整包塞进首页的
 *  First Load JS（514kB）。改成「抽屉打开才加载」后，首屏不欠这份钱。 */
export function AlmanacSection(props: {
  year: number;
  month: number;
  selected: string;
  today: string;
  doneMap: Record<string, number>;
  onMonth: (y: number, m: number) => void;
  onPick: (date: string) => void;
}) {
  return (
    <>
      <AlmanacCalendar {...props} />
      <AlmanacDetailPanel date={props.selected} />
    </>
  );
}
