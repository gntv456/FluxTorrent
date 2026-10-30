/**
 * 作品聚合的另外两种展示形态（列表 / 海报墙），与卡片形态共用
 * `/api/v1/works` 同一份数据。卡片（WorkCard）带卡内种子行，信息最重；
 * 这里只给作品粒度摘要：列表=一行一部作品，海报墙=封面网格。
 */

import Link from "next/link";
import { formatBytes } from "@/lib/format";
import { dateLocale, type Locale } from "@/i18n/config";
import { PosterImage } from "@/components/poster-image";
import { Icon } from "@/components/icons";
import { workTitle } from "@/components/work-chips";
import { workFlags } from "@/components/work-flags";
import type { WorkWithSeeds } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";

type Props = {
  items: WorkWithSeeds[];
  dict: Dict;
  locale: Locale;
  catName: (id: number) => string;
};

function workHref(key: string): string {
  return `/works/${key}`;
}

function workPoster(w: WorkWithSeeds): string | null {
  const meta = w.meta?.source ? w.meta : null;
  return meta?.poster ?? w.poster ?? null;
}

function workYear(w: WorkWithSeeds): number | null {
  const meta = w.meta?.source ? w.meta : null;
  return meta?.year ?? w.year;
}

function latestDate(w: WorkWithSeeds, locale: Locale): string {
  return new Date(w.latest_at)
    .toLocaleDateString(dateLocale(locale));
}

/** 列表形态：一行一部作品（封面缩略 + 片名/年份/分类/种子数 + 统计） */
export function WorkRows({ items, dict, locale, catName }: Props) {
  const t = dict.torrents;
  return (
    <ul className="wk-list">
      {items.map((w) => {
        const title = workTitle(w.name);
        const year = workYear(w);
        const flags = workFlags(w, dict);
        return (
          <li key={w.key} className="wk-lrow" data-work-key={w.key}>
            <Link href={workHref(w.key)} className="wk-lcover">
              <PosterImage
                src={workPoster(w)}
                alt=""
                size="sm"
                className="h-full w-full object-cover"
                fallback={
                  <span className="wk-lcover-fb" aria-hidden>
                    <Icon name="disc" size={16} />
                  </span>
                }
              />
            </Link>
            <div className="wk-lmain">
              <Link href={workHref(w.key)} className="wk-ltitle">
                {title}
              </Link>
              {(flags.qualities.length > 0 ||
                flags.promos.length > 0 ||
                flags.official ||
                flags.ratings.length > 0) && (
                <span className="wk-lflags">
                  {flags.qualities.slice(0, 1).map((q) => (
                    <span key={q} className="wk-flag wk-flag--quality">
                      {q}
                    </span>
                  ))}
                  {flags.promos.slice(0, 1).map((p) => (
                    <span
                      key={p.key}
                      className={`wk-flag wk-flag--${p.tone}`}
                    >
                      {p.label}
                    </span>
                  ))}
                  {flags.official && (
                    <span className="wk-flag wk-flag--official">
                      {dict.torrents.officialTag}
                    </span>
                  )}
                  {flags.ratings.slice(0, 1).map((r) => (
                    <span
                      key={r.label}
                      className="wk-flag wk-flag--rating"
                    >
                      ★ {r.label} {r.value}
                    </span>
                  ))}
                </span>
              )}
              <span className="wk-lmeta">
                {year ? (
                  <span className="wk-year">{year}</span>
                ) : null}
                <span>{catName(w.category_id)}</span>
                {flags.genres.length > 0 && (
                  <span>{flags.genres.slice(0, 2).join(" / ")}</span>
                )}
                {flags.region && <span>{flags.region}</span>}
                <span>
                  {dict.works.seedsInWork.replace(
                    "{n}",
                    String(w.torrents),
                  )}
                </span>
              </span>
            </div>
            <span className="wk-lnums num">
              <span title={t.colSeeders}>↑ {w.seeders}</span>
              <span title={t.colLeechers}>↓ {w.leechers}</span>
              <span title={t.colSize}>{formatBytes(w.size)}</span>
              <span className="wk-ldate">
                {latestDate(w, locale)}
              </span>
            </span>
          </li>
        );
      })}
    </ul>
  );
}

/** 海报墙：封面网格（分类角标 / 年份 / 片名 / 种子数与做种） */
export function WorkPosters({ items, dict, locale, catName }: Props) {
  return (
    <ul className="wk-wall">
      {items.map((w) => {
        const title = workTitle(w.name);
        const year = workYear(w);
        const date = latestDate(w, locale);
        const flags = workFlags(w, dict);
        return (
          <li
            key={w.key}
            className="wk-wall-it"
            data-work-key={w.key}
          >
            <Link
              href={workHref(w.key)}
              className="wk-wall-a"
              title={title}
            >
              <span className="wk-wall-poster">
                <PosterImage
                  src={workPoster(w)}
                  alt=""
                  size="lg"
                  className="h-full w-full object-cover"
                  sizes="(max-width: 640px) 45vw, 180px"
                  fallback={
                    <span className="wk-poster-t">{title}</span>
                  }
                />
                <span className="wk-eyebrow">
                  {catName(w.category_id)}
                </span>
                {year ? (
                  <span className="wk-wall-year">{year}</span>
                ) : null}
              </span>
              <span className="wk-wall-bar">
                <span className="wk-wall-t">{title}</span>
                {(flags.qualities.length > 0 ||
                  flags.promos.length > 0 ||
                  flags.official ||
                  flags.ratings.length > 0) && (
                  <span className="wk-wall-flags">
                    {flags.qualities.slice(0, 1).map((q) => (
                      <span
                        key={q}
                        className="wk-flag wk-flag--quality"
                      >
                        {q}
                      </span>
                    ))}
                    {flags.promos.slice(0, 1).map((p) => (
                      <span
                        key={p.key}
                        className={`wk-flag wk-flag--${p.tone}`}
                      >
                        {p.label}
                      </span>
                    ))}
                    {flags.official && (
                      <span className="wk-flag wk-flag--official">
                        {dict.torrents.officialTag}
                      </span>
                    )}
                    {flags.ratings.slice(0, 1).map((r) => (
                      <span
                        key={r.label}
                        className="wk-flag wk-flag--rating"
                      >
                        ★ {r.label} {r.value}
                      </span>
                    ))}
                  </span>
                )}
                <span className="wk-wall-m">
                  {flags.genres.length > 0
                    ? `${flags.genres.slice(0, 2).join(" / ")} · `
                    : ""}
                  {dict.works.seedsInWork.replace(
                    "{n}",
                    String(w.torrents),
                  )}
                  {" · ↑ "}
                  {w.seeders}
                  {" · "}
                  {formatBytes(w.size)}
                </span>
                <span className="wk-wall-d">
                  {dict.works.latestAt.replace("{t}", date)}
                </span>
              </span>
            </Link>
          </li>
        );
      })}
    </ul>
  );
}
