"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { MedalIcon } from "@/components/medal-icon";
import { MedalRarityChip } from "@/components/medal-rarity-chip";
import type { MedalRarity } from "@/lib/medal-rarity";

// hxpt 插件移植前台——勋章墙（从 plugins.tsx 按域拆出，
// 同文件还有 contests / frame-shop / gomoku 三个域）

interface MedalWallMedal {
  medal_name: string;
  asset_ref?: string | null;
  rarity?: string | null;
  wearing: boolean;
  granted_at: string;
}
interface MedalWallUser {
  user_id: number;
  username: string;
  medal_count: number;
  medals: MedalWallMedal[];
}
interface MedalWallPage {
  rows: MedalWallUser[];
  total: number;
  page: number;
  per_page: number;
}

/** 勋章墙主体：一卡一人（持勋数降序），卡内勋章网格 + 稀有度角标 + 佩戴高亮；
 *  支持用户名/勋章名搜索与按用户分页。稀有度词表由 RSC 侧传入（0143 口径，
 *  接口失败有兜底）。 */
export function MedalWall({ rarities }: { rarities: MedalRarity[] }) {
  const { dict } = useI18n();
  const t = dict.medalwall;
  const [data, setData] = useState<MedalWallPage | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
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
      .get<MedalWallPage>(`/api/v1/medal-wall?${params}`)
      .then((d) => {
        setData(d);
        setErr(null);
      })
      .catch((e) => {
        setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
      });
  }, [q, page, dict]);

  useEffect(load, [load]);

  const totalPages = Math.max(
    1,
    Math.ceil((data?.total ?? 0) / perPage),
  );
  return (
    <div className="flex flex-col gap-3">
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          setPage(1); // 翻回第 1 页再触发 load（q/page 都是依赖）
        }}
      >
        <input
          id="medalwall-q"
          className="min-h-[40px] flex-1 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-3 text-sm"
          placeholder={t.searchHint}
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <button
          type="submit"
          className="min-h-[40px] rounded-full bg-sky px-4 text-sm font-bold text-white active:scale-[0.97]"
        >
          {t.search}
        </button>
      </form>

      {err && <p className="baozi-panel p-4 text-sm text-sub">{err}</p>}
      {!err && data === null && (
        <p className="baozi-panel p-4 text-sm text-sub">{t.loading}</p>
      )}
      {!err && data?.rows.length === 0 && (
        <p className="baozi-panel p-4 text-sm text-sub">{t.empty}</p>
      )}

      {data && data.rows.length > 0 && (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-3">
          {data.rows.map((u) => (
            <div
              key={u.username}
              className="baozi-panel flex flex-col gap-3 p-4"
            >
              <div className="flex items-center justify-between gap-2">
                <a
                  href={`/users/${u.user_id}`}
                  className="truncate font-bold text-ink hover:underline"
                >
                  {u.username}
                </a>
                <span className="num shrink-0 text-xs text-sub">
                  {fmt(t.count, { n: u.medal_count })}
                </span>
              </div>
              <div className="flex flex-wrap gap-2">
                {u.medals.map((m) => (
                  <span
                    key={m.medal_name}
                    className={`medalwall-medal ${m.wearing ? "medalwall-worn" : ""}`}
                    title={`${m.medal_name}${m.wearing ? ` · ${t.wearing}` : ""}`}
                  >
                    <MedalIcon
                      src={m.asset_ref}
                      size={30}
                      title={m.medal_name}
                    />
                    {m.rarity && (
                      <MedalRarityChip
                        list={rarities}
                        value={m.rarity}
                        className="medalwall-rarity"
                      />
                    )}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {data && data.total > perPage && (
        <div className="flex items-center justify-between text-sm text-sub">
          <span>{fmt(t.totalUsers, { n: data.total })}</span>
          <div className="flex items-center gap-2">
            <button
              disabled={page <= 1}
              onClick={() => setPage(page - 1)}
              className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
            >
              {t.prev}
            </button>
            <span className="num">
              {page} / {totalPages}
            </span>
            <button
              disabled={page >= totalPages}
              onClick={() => setPage(page + 1)}
              className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
            >
              {dict.common.nextPage}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
