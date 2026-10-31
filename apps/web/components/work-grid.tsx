/**
 * 作品聚合「网格」形态（Colapt 网格口径）：一屏 2~3 部作品，
 * 卡片 = 迷你海报头（片名/年份/官方/评分/类型）+ 前两条种子 +
 * 「其余 N 个版本」。
 *
 * 与「宽松」共用 `/api/v1/works` 同一份数据（作品行 + 前 8 条种子），
 * 不额外请求；种子行只取前 2 条，其余走作品详情页。
 * 服务端组件；种子行交给 WorkSeedMini（无 hooks）。
 */

import Link from "next/link";
import { PosterImage } from "@/components/poster-image";
import { workTitle } from "@/components/work-chips";
import { workFlags } from "@/components/work-flags";
import { WorkSeedMini } from "@/components/work-seed-mini";
import type { WorkWithSeeds } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";
import type { Locale } from "@/i18n/config";

/** 网格卡里挂几条种子（其余给「其余 N 个版本」） */
const MAX_SEEDS = 2;

export function WorkGrid({
  items,
  dict,
  locale,
  catName,
}: {
  items: WorkWithSeeds[];
  dict: Dict;
  locale: Locale;
  catName: (id: number) => string;
}) {
  // 倒计时基准一次定死（服务端渲染时刻），卡片内所有种子行共用
  const now = Date.now();
  return (
    <ul className="wk-grid">
      {items.map((w) => {
        const title = workTitle(w.name);
        const href = `/works/${w.key}`;
        const meta = w.meta?.source ? w.meta : null;
        const year = meta?.year ?? w.year;
        const poster = meta?.poster ?? w.poster ?? null;
        const flags = workFlags(w, dict);
        const shown = w.seeds.slice(0, MAX_SEEDS);
        const rest = Math.max(0, w.torrents - shown.length);
        return (
          <li key={w.key} className="wk-grid-it" data-work-key={w.key}>
            <article className="wk-gcard">
              <header className="wk-ghead">
                <Link
                  href={href}
                  className="wk-gposter"
                  aria-label={title}
                >
                  <PosterImage
                    src={poster}
                    alt=""
                    size="sm"
                    className="h-full w-full object-cover"
                    fallback={null}
                  />
                </Link>
                <div className="wk-ginfo">
                  <h3 className="wk-gtitle">
                    <Link href={href}>{title}</Link>
                  </h3>
                  {(year ||
                    flags.official ||
                    flags.ratings.length > 0) && (
                    <div className="wk-grow">
                      {year ? (
                        <span className="wk-year">{year}</span>
                      ) : null}
                      {flags.official && (
                        <span className="wk-flag wk-flag--official">
                          {dict.torrents.officialTag}
                        </span>
                      )}
                      {flags.ratings.map((r) => (
                        <span key={r.label} className="wk-grate">
                          ★ {r.label} {r.value}
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="wk-gtags">
                    {flags.genres.slice(0, 3).map((g) => (
                      <span key={g}>{g}</span>
                    ))}
                    <span className="wk-gcount">
                      {dict.works.seedsInWork.replace(
                        "{n}",
                        String(w.torrents),
                      )}
                    </span>
                  </div>
                </div>
              </header>
              <WorkSeedMini
                seeds={shown}
                dict={dict}
                locale={locale}
                now={now}
              />
              <footer className="wk-gfoot">
                <Link href={href} className="wk-gmore">
                  {rest > 0
                    ? dict.works.moreVersions.replace(
                        "{n}",
                        String(rest),
                      )
                    : dict.works.openWork}{" "}
                  ›
                </Link>
              </footer>
            </article>
          </li>
        );
      })}
    </ul>
  );
}
