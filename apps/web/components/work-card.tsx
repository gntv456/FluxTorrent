/**
 * 作品卡（资源页「作品聚合」视图，Colapt「宽松」式媒体行）：
 *   左：海报 + 分类角标 + 最新日期；
 *   右：片名/年份/官方/条目 ID/评分/新增种子 → 地区/导演/片长 →
 *       类型 + 标签 → 简介 → 主演 → 「匹配的种子版本」四列种子行。
 *
 * 数据全部来自 `/api/v1/works`（作品行 + 前 8 条种子），不额外请求。
 * 服务端组件；种子行交给 WorkSeedRows（详情页复用同一实现）。
 */

import Link from "next/link";
import { dateLocale, type Locale } from "@/i18n/config";
import { PosterImage } from "@/components/poster-image";
import { WorkSeedRows } from "@/components/work-seed-rows";
import { WorkIds } from "@/components/work-ids";
import { workTitle } from "@/components/work-chips";
import { workFlags } from "@/components/work-flags";
import type { WorkWithSeeds } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";

/** 「添加种子」落点：带作品名，上传页预填种子名（便于同作品标题归一化）。 */
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
  const poster = meta?.poster ?? work.poster ?? null;
  const date = new Date(work.latest_at).toLocaleDateString(dateLocale(locale));
  const syn = (meta?.synopsis ?? work.synopsis ?? "").trim();
  const flags = workFlags(work, dict);
  const genreTags = flags.genres.slice(0, 4);
  const userTags = work.tags ?? [];
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
          {flags.official && (
            <span className="wk-flag wk-flag--official">
              {dict.torrents.officialTag}
            </span>
          )}
          <WorkIds work={work} dict={dict} />
          <span className="wk-head-actions">
            {flags.ratings.length > 0 && (
              <span className="wk-head-ratings">
                {flags.ratings.map((r) => (
                  <span
                    key={r.label}
                    className="wk-flag wk-flag--rating"
                  >
                    ★ {r.label} {r.value}
                  </span>
                ))}
              </span>
            )}
            <Link href={uploadHref(work)} className="wk-add">
              ＋ {dict.works.addSeed}
            </Link>
          </span>
        </div>

        <div className="wk-media-meta">
          {[
            catName,
            flags.region,
            flags.director.length > 0
              ? `${dict.works.director} ${flags.director
                  .slice(0, 2)
                  .join(" / ")}`
              : null,
            flags.runtime
              ? `${flags.runtime} ${dict.works.min}`
              : null,
          ]
            .filter(Boolean)
            .join(" · ")}
        </div>

        {(flags.qualities.length > 0 || flags.promos.length > 0) && (
          <div className="wk-flags">
            {flags.qualities.map((q) => (
              <span key={q} className="wk-flag wk-flag--quality">
                {q}
              </span>
            ))}
            {flags.promos.map((p) => (
              <span
                key={p.key}
                className={`wk-flag wk-flag--${p.tone}`}
              >
                {p.label}
              </span>
            ))}
          </div>
        )}

        {(genreTags.length > 0 || userTags.length > 0) && (
          <div className="wk-tags">
            {genreTags.map((g) => (
              <span key={`g:${g}`} className="wk-genre">
                {g}
              </span>
            ))}
            {userTags.slice(0, 4).map((b) => (
              <span key={b.id}>{b.name}</span>
            ))}
          </div>
        )}

        <p className="wk-syn">{syn || dict.works.noSynopsis}</p>

        {flags.cast.length > 0 && (
          <div className="wk-crew">
            <div>
              <b>{dict.works.cast}</b>
              {flags.cast.slice(0, 5).join(" / ")}
            </div>
          </div>
        )}

        <div className="wk-versions">
          <div className="wk-versions-head">
            <span className="wk-versions-t">{dict.works.versions}</span>
            <span className="wk-versions-n">
              {dict.works.seedsInWork.replace(
                "{n}",
                String(work.torrents),
              )}
            </span>
          </div>
          <WorkSeedRows
            items={work.seeds}
            dict={dict}
            locale={locale}
            total={work.torrents}
            moreHref={href}
            variant="relaxed"
          />
        </div>
      </div>
    </article>
  );
}
