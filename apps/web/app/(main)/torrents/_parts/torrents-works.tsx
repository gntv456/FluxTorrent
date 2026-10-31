/**
 * 资源页「作品聚合」视图（样图 design/资源页-影视资料库样图.html 的 group 模式）：
 * 按作品归堆，展示形态与 Colapt 三档一一对应——
 *   · 列表 = 宽松：每部作品一条媒体行（海报 + 简介 + 七列种子表）
 *   · 卡片 = 网格：2~3 列作品卡，各挂前两条种子 + 「其余 N 个版本」
 *   · 海报墙 = 瀑布：整面封面墙，点封面就地展开版本面板
 *
 * 取数独立端点 `/api/v1/works`：作品聚合要按归一化标题全表聚合，塞进
 * `/torrents` 会拖慢普通列表（后端注释见 works_sql.rs）。
 *
 * 与列表页其余视图共用 URL 参数：p（页码）、search、分类、标签、维度、
 * 数值区间、促销、视角维度与 sort；approval 除外（作品层只展示过审资源）。
 */

import Link from "next/link";
import { api } from "@/lib/api-client";
import { ErrorPanel } from "@/components/ui/error-panel";
import { normalizeError } from "@/lib/render-error";
import { WorkCard } from "@/components/work-card";
import { WorkGrid } from "@/components/work-grid";
import { WorkWall } from "@/components/work-wall";
import type { WallItem } from "@/components/work-wall";
import { workFlags } from "@/components/work-flags";
import { workTitle } from "@/components/work-chips";
import { dateLocale } from "@/i18n/config";
import type { WorkPage, WorkWithSeeds } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";
import type { Locale } from "@/i18n/config";
import type { TorrentsSP, TorrentView } from "./torrents-utils";

/** 作品聚合与种子列表共用同一套筛选参数；approval 不在此列——
 *  作品层只展示过审资源（后端会拒绝 approval 视图）。 */
const FILTER_KEYS = [
  "category_id",
  "official",
  "search_area",
  "search_mode",
  "alive",
  "status",
  "tag_mode",
  "size_min",
  "size_max",
  "date_from",
  "date_to",
  "min_seeders",
  "max_seeders",
  "exclude",
  "promo",
  "owner",
  "mine",
  "bookmarked",
  "min_leechers",
  "max_leechers",
  "min_completed",
  "max_completed",
  "anonymous",
  "rating_min",
] as const;

export async function TorrentWorks({
  dict,
  locale,
  sp,
  cats,
  view,
  perPage,
  withParam,
}: {
  dict: Dict;
  locale: Locale;
  sp: TorrentsSP;
  cats: { id: number; label: string }[];
  view: TorrentView;
  perPage: number;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
}) {
  const pageNo = Math.max(1, Number(sp.p ?? "1") || 1);
  const qs = new URLSearchParams({
    limit: String(perPage),
    page: String(pageNo),
  });
  if (sp.search) qs.set("q", sp.search);
  if (sp.sort) qs.set("sort", sp.sort);
  for (const key of FILTER_KEYS) {
    const v = sp[key];
    if (v) qs.set(key, v);
  }
  const tagIds = sp.tag_ids ?? sp.tag_id;
  if (tagIds) qs.set("tag_ids", tagIds);
  for (const [k, v] of Object.entries(sp)) {
    if (k.startsWith("sec_") && v) qs.set(k, v);
  }

  let data: WorkPage | null = null;
  let err: unknown = null;
  try {
    data = await api.get<WorkPage>(`/api/v1/works?${qs.toString()}`);
  } catch (e) {
    err = e;
  }

  if (err !== null) {
    return (
      <ErrorPanel
        error={normalizeError(err)}
        desc={dict.torrents.loadFailedHint}
        retryHref={withParam(sp, "p", String(pageNo))}
      />
    );
  }
  if (!data || data.items.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-sub">
        {dict.works.empty}
      </p>
    );
  }

  const catName = (id: number) =>
    cats.find((c) => c.id === id)?.label ?? "";
  const totalPages = Math.max(1, Math.ceil(data.total / perPage));
  // 瀑布只把墙与面板要用的列交给客户端组件（不带 tags/sec_names 等）
  const toWallItem = (w: WorkWithSeeds): WallItem => {
    const meta = w.meta?.source ? w.meta : null;
    const flags = workFlags(w, dict);
    return {
      key: w.key,
      title: workTitle(w.name),
      poster: meta?.poster ?? w.poster ?? null,
      year: meta?.year ?? w.year,
      cat: catName(w.category_id),
      quality: flags.qualities[0] ?? null,
      promo: flags.promos[0]?.label ?? null,
      ratings: flags.ratings,
      torrents: w.torrents,
      seeders: w.seeders,
      latest: new Date(w.latest_at).toLocaleDateString(
        dateLocale(locale),
      ),
      seeds: w.seeds.map((s) => ({
        id: s.id,
        name: s.name,
        size: s.size,
        seeders: s.seeders,
        leechers: s.leechers,
        created_at: s.created_at,
        promotion: s.promotion,
        official: s.official,
      })),
    };
  };

  return (
    <div className="flex flex-col gap-2">
      <p className="wk-lead">
        {dict.works.countWorks.replace("{n}", String(data.total))}
      </p>
      {view === "table" ? (
        data.items.map((w) => (
          <WorkCard
            key={w.key}
            work={w}
            dict={dict}
            locale={locale}
            catName={catName(w.category_id)}
          />
        ))
      ) : view === "card" ? (
        <WorkGrid
          items={data.items}
          dict={dict}
          locale={locale}
          catName={catName}
        />
      ) : (
        <WorkWall items={data.items.map(toWallItem)} />
      )}
      {totalPages > 1 && (
        <nav className="flex items-center justify-center gap-3 py-1">
          {pageNo > 1 && (
            <Link
              href={withParam(sp, "p", String(pageNo - 1))}
              className="wk-pg"
            >
              {dict.common.prevPage}
            </Link>
          )}
          <span className="text-sm text-sub">
            {dict.common.pageX.replace("{n}", String(pageNo))}
            {" / "}
            {totalPages}
          </span>
          {pageNo < totalPages && (
            <Link
              href={withParam(sp, "p", String(pageNo + 1))}
              className="wk-pg"
            >
              {dict.common.nextPage}
            </Link>
          )}
        </nav>
      )}
    </div>
  );
}
