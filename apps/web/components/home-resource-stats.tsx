"use client";

import { useI18n } from "@/i18n/client";
import type { Home2T, HomeData } from "@/components/home-data";

/** 首页资源统计板块（从 home-sections.tsx 按域拆出，300 门禁）：
 *  30 天新增资源堆叠柱状图 + 今日/7 日均/30 日累计指标。 */

export function ResourceStatsPanel({
  series,
  t,
}: {
  series: HomeData["resource_stats"];
  t: Home2T;
}) {
  const max = Math.max(1, ...series.series.map((d) => d.total));
  return (
    <section className="baozi-panel home-resource-stats">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">▥</span> {t.statsTitle}
        </h2>
        <small>{t.statsNote}</small>
      </header>
      <div className="home-resource-stats__metrics">
        <div className="home-resource-stats__metric">
          <span>{t.statsToday}</span>
          <strong className="num">{series.today}</strong>
          <small>{t.statsAsOfNow}</small>
        </div>
        <div className="home-resource-stats__metric">
          <span>{t.statsAvg7}</span>
          <strong className="num">{series.avg7.toFixed(1)}</strong>
          <small>{t.statsExclToday}</small>
        </div>
        <div className="home-resource-stats__metric">
          <span>{t.statsTotal30}</span>
          <strong className="num">{series.total30.toLocaleString()}</strong>
          <small>{t.statsResource}</small>
        </div>
        <div className="home-resource-stats__legend">
          <span>
            <i className="legend-swatch legend-swatch--ordinary" aria-hidden="true" />{" "}
            {t.statsOrdinary}
          </span>
          <span>
            <i className="legend-swatch legend-swatch--official" aria-hidden="true" /> {t.statsOfficial}
          </span>
        </div>
      </div>
      <figure className="home-resource-stats__figure">
        <div className="home-resource-stats__chart" role="img" aria-label={t.statsChartAria}>
          {series.series.map((d) => (
            <div key={d.date} className="rs-bar" title={`${d.date}：${t.statsOrdinary} ${d.ordinary}，${t.statsOfficial} ${d.official}，${t.statsTotalShort} ${d.total}`}>
              {d.official > 0 && (
                <i className="rs-bar__official" style={{ flexGrow: d.official / max }} />
              )}
              <i className="rs-bar__ordinary" style={{ flexGrow: d.ordinary / max }} />
            </div>
          ))}
        </div>
      </figure>
    </section>
  );
}
