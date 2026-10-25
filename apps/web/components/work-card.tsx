/**
 * 作品卡（资源页「作品聚合」视图的一张卡，样图同一结构）：
 *   ┌ 封面列（海报 + 分类角标 + 最新日期徽章）
 *   └ 主区（片名 / 年份 / 分类 / 种子数 / 添加种子 → 标签 → 简介 →
 *           统计 → 该作品的种子行）
 *
 * 数据全部来自 `/api/v1/works`（作品行 + 前 8 条种子），不额外请求。
 * 服务端组件；行渲染交给 WorkSeedRows（同一份实现给作品详情页复用）。
 */

import Link from "next/link";
import { formatBytes } from "@/lib/format";
import { dateLocale, type Locale } from "@/i18n/config";
import { PosterImage } from "@/components/poster-image";
import { WorkSeedRows } from "@/components/work-seed-rows";
import { WorkIds } from "@/components/work-ids";
import { workTitle } from "@/components/work-chips";
import type { WorkWithSeeds } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";

/** 「添加种子」落点：带作品名，上传页可预填组名（P-?? 由上传页解析） */
function uploadHref(work: WorkWithSeeds): string {
  return `/upload?group=${encodeURIComponent(workTitle(work.name))}`;
}

export function WorkCard({
  work,
  dict,
  locale,
  catName,
}: {
  work: WorkWithSeeds;
  dict: Dict;
  locale: Locale;
  catName: string;
}) {
  const title = workTitle(work.name);
  const href = `/works/${work.key}`;
  // 作品层缓存（work_meta）优先：TMDB 拉回的年份/海报比「片名抓年份」可靠
  const meta = work.meta?.source ? work.meta : null;
  const year = meta?.year ?? work.year;
  const poster = work.poster ?? meta?.poster ?? null;
  const date = new Date(work.latest_at).toLocaleDateString(dateLocale(locale));
  const syn = (work.synopsis ?? "").trim();
  return (
    <article className="wk-card" data-work-key={work.key}>
      <div className="wk-cover">
        <Link href={href} className="wk-poster" aria-label={title}>
          <PosterImage
            src={poster}
            alt=""
            size="lg"
            className="wk-poster-img h-full w-full object-cover"
            // 无海报时站在 .wk-poster 的渐变底上，用片名当封面字（样图口径：
            // 生成式海报），而不是全站默认的「占位底 + 唱片图标」
            fallback={<span className="wk-poster-t">{title}</span>}
          />
          <span className="wk-eyebrow">{catName}</span>
          <span className="wk-poster-t">{title}</span>
        </Link>
        <div className="wk-cover-note">
          {title}
          <br />
          <span className="wk-date">{date}</span>
        </div>
      </div>

      <div className="wk-main">
        <div className="wk-head">
          <h3 className="wk-title">
            <Link href={href}>{title}</Link>
          </h3>
          {year ? <span className="wk-year">{year}</span> : null}
          <span className="wk-cat">{catName}</span>
          <span className="wk-stat">
            {dict.works.seedsInWork.replace("{n}", String(work.torrents))}
          </span>
          {meta?.rating ? (
            <span className="wk-stat">
              ★ {meta.rating.toFixed(1)}
            </span>
          ) : null}
          <WorkIds work={work} dict={dict} />
          <Link href={uploadHref(work)} className="wk-add">
            ＋ {dict.works.addSeed}
          </Link>
        </div>

        {(work.tags?.length ?? 0) > 0 && (
          <div className="wk-tags">
            {work.tags.slice(0, 6).map((b) => (
              <span key={b.id}>{b.name}</span>
            ))}
          </div>
        )}

        <p className="wk-syn">{syn || dict.works.noSynopsis}</p>

        <div className="wk-stats">
          <span>
            {dict.torrents.colSeeders}{" "}
            <b>{work.seeders}</b>
          </span>
          <span>
            {dict.torrents.colLeechers}{" "}
            <b>{work.leechers}</b>
          </span>
          <span>
            {dict.torrents.colSize}{" "}
            <b>{formatBytes(work.size)}</b>
          </span>
          <span>
            {dict.works.latestAt.replace("{t}", date)}
          </span>
        </div>

        <WorkSeedRows
          items={work.seeds}
          dict={dict}
          locale={locale}
          total={work.torrents}
          moreHref={href}
        />
      </div>
    </article>
  );
}
