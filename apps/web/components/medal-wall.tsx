"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { BTN_SM_SQUARE, INPUT_MD } from "@/lib/ui-classes";
import type { MedalRarity } from "@/lib/medal-rarity";
import {
  HonorCard,
  RarityChips,
  WallStats,
  type WallUser,
} from "@/components/medal-wall-parts";

// 勋章墙主体（medal_wall.php 口径）：一卡一人、持勋数降序。
// 改版要点（2026-09-23）：统计条 + 稀有度筛选 + 排序 + 分布条 + 徽章槽位悬停档案。
// 口径诚实原则：统计条四格分别是「勋章种类（medals 接口）/ 稀有度档位（词表）/
// 持勋用户（服务端 total）/ 本页枚次（当前页汇总）」——后两者标了单位，不混口径。

interface WallPage {
  rows: WallUser[];
  total: number;
  page: number;
  per_page: number;
}

export function MedalWall({
  rarities,
  medalKinds,
}: {
  rarities: MedalRarity[];
  /** 全站勋章种类数（RSC 侧由 /medals 得来，比墙接口更准） */
  medalKinds: number;
}) {
  const { dict, locale } = useI18n();
  const t = dict.medalwall;
  const [data, setData] = useState<WallPage | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [rar, setRar] = useState<string | null>(null);
  const [sort, setSort] = useState("count");
  const perPage = 12;

  const load = useCallback(() => {
    const params = new URLSearchParams({
      page: String(page),
      per_page: String(perPage),
    });
    const kw = q.trim();
    if (kw.length >= 2) params.set("q", kw);
    setData(null);
    api
      .get<WallPage>(`/api/v1/medal-wall?${params}`)
      .then((d) => {
        setData(d);
        setErr(null);
      })
      .catch((e) => {
        setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
      });
  }, [q, page, dict]);

  useEffect(load, [load]);

  // 稀有度筛选选中后：卡内只显示该档，且没有该档的卡整张不渲染
  const rows = useMemo(() => {
    const list = data?.rows ?? [];
    if (sort !== "recent") return list;
    return [...list].sort((a, b) => {
      const ka = a.medals.map((m) => m.granted_at).sort().pop() ?? "";
      const kb = b.medals.map((m) => m.granted_at).sort().pop() ?? "";
      return kb.localeCompare(ka);
    });
  }, [data, sort]);

  const counts = useMemo(() => {
    const c: Record<string, number> = {};
    for (const u of data?.rows ?? []) {
      for (const m of u.medals) {
        if (m.rarity) c[m.rarity] = (c[m.rarity] ?? 0) + 1;
      }
    }
    return c;
  }, [data]);

  const pageMedals = useMemo(
    () => (data?.rows ?? []).reduce((n, u) => n + u.medals.length, 0),
    [data],
  );

  const visible = rows.filter(
    (u) => !rar || u.medals.some((m) => m.rarity === rar),
  );
  const totalPages = Math.max(1, Math.ceil((data?.total ?? 0) / perPage));
  const rankBase = (page - 1) * perPage;

  return (
    <div className="flex flex-col gap-3">
      <WallStats
        kinds={medalKinds}
        tiers={rarities.length}
        users={data?.total ?? 0}
        pageMedals={pageMedals}
        t={t}
      />

      <form
        className="pgtools"
        onSubmit={(e) => {
          e.preventDefault();
          setPage(1);
        }}
      >
        <input
          id="medalwall-q"
          className={`${INPUT_MD} min-w-[220px] flex-1 text-sm`}
          placeholder={t.searchHint}
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <button type="submit" className={BTN_SM_SQUARE}>
          {t.search}
        </button>
        <RarityChips
          list={rarities}
          counts={counts}
          value={rar}
          onChange={setRar}
          allLabel={t.filterAll}
          total={pageMedals}
        />
        <select
          className="pgsel ml-auto"
          value={sort}
          onChange={(e) => setSort(e.target.value)}
          aria-label={t.sortLabel}
        >
          <option value="count">{t.sortCount}</option>
          <option value="recent">{t.sortRecent}</option>
        </select>
      </form>

      {err && <p className="baozi-panel p-4 text-sm text-sub">{err}</p>}
      {!err && data === null && (
        <p className="baozi-panel p-4 text-sm text-sub">{t.loading}</p>
      )}
      {!err && data?.rows.length === 0 && (
        <p className="baozi-panel p-4 text-sm text-sub">{t.empty}</p>
      )}
      {!err && data !== null && data.rows.length > 0 && visible.length === 0 && (
        <p className="baozi-panel p-4 text-sm text-sub">{t.emptyFilter}</p>
      )}

      {rows.length > 0 && (
        <div className="honor-grid">
          {rows.map((u, i) => (
            <HonorCard
              key={u.user_id}
              user={u}
              rank={rankBase + i + 1}
              list={rarities}
              t={t}
              locale={locale}
              only={rar}
            />
          ))}
        </div>
      )}

      {data && data.total > perPage && (
        <div className="flex items-center justify-between gap-3 flex-wrap">
          <span className="text-sm text-sub">
            {t.totalUsers.replace("{n}", String(data.total))}
          </span>
          <div className="tide-pager">
            <button
              type="button"
              className="tide-pager__btn"
              disabled={page <= 1}
              onClick={() => setPage(page - 1)}
            >
              {t.prev}
            </button>
            <span className="tide-pager__btn" aria-current="page">
              {page} / {totalPages}
            </span>
            <button
              type="button"
              className="tide-pager__btn"
              disabled={page >= totalPages}
              onClick={() => setPage(page + 1)}
            >
              {dict.common.nextPage}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
