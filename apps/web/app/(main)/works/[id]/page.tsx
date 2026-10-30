/**
 * 作品详情（样图「作品详情」视图）：一部作品的门面 + 它的全部种子 + 相关推荐。
 *
 * 与 `/torrents` 列表页共享视觉语言（作品卡 = 列表里的卡 + 展开），数据来自
 * `GET /api/v1/works/{key}`（作品头 + 种子行 + 同分类相关作品，一次取齐）。
 *
 * 不改变种子详情（/torrent/{id}）：那一页仍按原样渲染，本页只做「作品」这一层。
 */

import "@/app/styles/works.css";
import { notFound } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { formatBytes } from "@/lib/format";
import { dateLocale } from "@/i18n/config";
import { PosterImage } from "@/components/poster-image";
import { WorkRelatedGrid } from "@/components/work-related-grid";
import { WorkSeedRows } from "@/components/work-seed-rows";
import { WorkIds } from "@/components/work-ids";
import { WorkWatchButton } from "@/components/work-watch-button";
import { workTitle } from "@/components/work-chips";
import type { WorkDetail } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

/** 相关推荐上限：详情口只给 6 条，宽屏一排放得下更多，补到 12 条 */
const RELATED_MAX = 12;

interface CatRow {
  id: number;
  name: string;
  parent_id?: number | null;
}

/** 分类名（站型包单源；取不到就留空，不猜）+ Hero 磨砂参数（0400） */
async function profile(): Promise<{
  cats: Record<number, string>;
  hero: { blur: number; dim: number };
  wishlistOn: boolean;
}> {
  const fallback = {
    cats: {},
    hero: { blur: 28, dim: 0.55 },
    wishlistOn: false,
  };
  try {
    const r = await api.get<{
      categories?: CatRow[];
      work_hero?: { blur?: number; dim?: number };
      modules?: Record<string, boolean>;
    }>("/api/v1/site-profile");
    return {
      cats: Object.fromEntries(
        (r.categories ?? []).map((c) => [c.id, c.name]),
      ),
      hero: {
        blur: Number(r.work_hero?.blur ?? fallback.hero.blur),
        dim: Number(r.work_hero?.dim ?? fallback.hero.dim),
      },
      // 订阅写的是愿望单：模块关闭时后端回 4101，按钮也不该出现
      wishlistOn: r.modules?.wishlist !== false,
    };
  } catch {
    return fallback;
  }
}

export default async function WorkDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const { dict, locale } = await getDict();
  const [data, prof] = await Promise.all([
    api.get<WorkDetail>(`/api/v1/works/${id}`).catch((e) => {
      // 只有「确实没有这部作品」才 404；接口抖动交给错误边界，不伪装成不存在
      if (e instanceof ApiError && (e.status === 404 || e.code === 1004)) {
        notFound();
      }
      throw e;
    }),
    profile(),
  ]);

  const w = data.work;
  // 详情口的 related 写死 6 条；用现成的作品列表口按同分类/同排序
  // （seeders DESC, latest_at DESC，与 related_sql 一致）补到上限。
  const related = [...data.related];
  if (related.length < RELATED_MAX) {
    try {
      const more = await api.get<{ items: WorkDetail["related"] }>(
        `/api/v1/works?category_id=${w.category_id}` +
          `&sort=seeders&limit=${RELATED_MAX + 1}`,
      );
      const seen = new Set([w.key, ...related.map((r) => r.key)]);
      for (const it of more.items) {
        if (related.length >= RELATED_MAX) break;
        if (seen.has(it.key)) continue;
        seen.add(it.key);
        related.push(it);
      }
    } catch {
      // 补位失败不拖垮主体：详情口那 6 条照常显示
    }
  }
  // 作品层缓存（work_meta）优先：TMDB 拉回的年份/海报/简介最完整
  const meta = w.meta?.source ? w.meta : null;
  const year = meta?.year ?? w.year;
  const poster = meta?.poster ?? w.poster ?? null;
  const rating = meta?.rating != null ? meta.rating.toFixed(1) : w.rating;
  const crew = [
    [dict.works.director, meta?.director],
    [dict.works.cast, meta?.cast_list],
  ] as const;
  const title = workTitle(w.name);
  const cat = prof.cats[w.category_id] ?? "";
  // Hero 磨砂底图（0400）：海报模糊 + 遮罩深度，后台「外观与主题」可调
  const heroBg = poster
    ? { backgroundImage: `url(${poster})`, filter: `blur(${prof.hero.blur}px)` }
    : undefined;
  const syn = (meta?.synopsis ?? w.synopsis ?? "").trim();
  const detail = (w.detail ?? meta?.synopsis ?? "").trim();
  const latest = new Date(w.latest_at).toLocaleDateString(dateLocale(locale));

  return (
    <div className="flex flex-col">
      <div className="wk-back">
        <Link href="/torrents?group=1&view=card">
          {`‹ ${dict.works.back}`}
        </Link>
      </div>

      <section className="wk-hero">
        {/* 磨砂底图（0400）：海报放大 + 高斯模糊，再压一层深色遮罩；没有
            海报就两层都不渲染，保持纯渐变 Hero */}
        {poster ? (
          <>
            <div className="wk-hero-bg" style={heroBg} aria-hidden />
            <div
              className="wk-hero-dim"
              style={{ opacity: prof.hero.dim }}
              aria-hidden
            />
          </>
        ) : null}
        <div className="wk-hero-in">
          <div className="wk-hero-poster">
            <PosterImage
              src={poster}
              alt=""
              size="lg"
              className="h-full w-full object-cover"
              fallback={<span className="wk-poster-t">{title}</span>}
            />
          </div>
          <div className="wk-hero-info">
            <div className="wk-hero-title">
              <span>{title}</span>
              {year ? <span className="wk-year">{year}</span> : null}
              {rating ? <span>★ {rating}</span> : null}
              <WorkIds work={w} dict={dict} />
            </div>
            <div className="wk-hero-meta">
              {[
                cat,
                meta?.genres?.filter(Boolean).join(" / "),
                meta?.runtime
                  ? `${meta.runtime} ${dict.works.min}`
                  : null,
                meta?.region,
                dict.works.seedsInWork.replace("{n}", String(w.torrents)),
                dict.works.latestAt.replace("{t}", latest),
              ]
                .filter(Boolean)
                .join(" · ")}
            </div>
            {(w.tags?.length ?? 0) > 0 && (
              <div className="wk-hero-tags">
                {w.tags.map((b) => (
                  <span key={b.id}>{b.name}</span>
                ))}
              </div>
            )}
            {crew.some(([, v]) => v?.length) ? (
              <div className="wk-crew">
                {crew
                  .filter(([, v]) => v?.length)
                  .map(([label, v]) => (
                    <div key={label}>
                      <b>{label}</b>
                      {v!.slice(0, 5).join(" / ")}
                    </div>
                  ))}
              </div>
            ) : null}
            <p className="wk-hero-syn">{syn || dict.works.noSynopsis}</p>
          </div>
          <div className="wk-meta-box">
            <div className="row">
              <span>{dict.torrents.colSeeders}</span>
              <b>{w.seeders}</b>
            </div>
            <div className="row">
              <span>{dict.torrents.colLeechers}</span>
              <b>{w.leechers}</b>
            </div>
            <div className="row">
              <span>{dict.torrents.colSize}</span>
              <b>{formatBytes(w.size)}</b>
            </div>
            {rating ? (
              <div className="row">
                <span>{dict.works.rating}</span>
                <b>{rating}</b>
              </div>
            ) : null}
            {year ? (
              <div className="row">
                <span>{dict.works.yearLabel}</span>
                <b>{year}</b>
              </div>
            ) : null}
            {meta?.runtime ? (
              <div className="row">
                <span>{dict.works.runtime}</span>
                <b>{`${meta.runtime} ${dict.works.min}`}</b>
              </div>
            ) : null}
            {meta?.region ? (
              <div className="row">
                <span>{dict.works.region}</span>
                <b>{meta.region}</b>
              </div>
            ) : null}
            <div className="row">
              <span>{dict.works.latestAt.replace("{t}", "")}</span>
              <b>{latest}</b>
            </div>
            {prof.wishlistOn && (
              <WorkWatchButton
                workKey={w.key}
                initialWatching={data.watching}
              />
            )}
          </div>
        </div>
      </section>

      {detail && (
        <section className="wk-panel">
          <div className="wk-panel-t">{dict.works.fullSynopsis}</div>
          {/* 完整简介按纯文本渲染（React 自动转义）：descr 可能含用户输入的
              HTML/BBCode，作品页不做富文本注水——种子详情页才是它的正式渲染位 */}
          <p className="wk-full">{detail}</p>
        </section>
      )}

      <section className="wk-panel">
        <div className="wk-panel-t">
          {dict.works.inWork}
          <span className="wk-count">{` · ${w.torrents}`}</span>
        </div>
        <WorkSeedRows
          items={data.items}
          dict={dict}
          locale={locale}
        />
      </section>

      {related.length > 0 && (
        <section className="wk-panel">
          <div className="wk-panel-t">{dict.works.related}</div>
          <WorkRelatedGrid items={related} cats={prof.cats} />
        </section>
      )}
    </div>
  );
}
