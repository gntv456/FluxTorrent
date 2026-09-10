import Link from "next/link";
import { api, paged } from "@/lib/api-client";
import { TorrentTr } from "@/components/torrent-table";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import type { Page, TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

/** 首页（包子站 index.php 复刻）：欢迎块 + 站点统计 + 最新种子表格 */
export default async function HomePage() {
  const { dict, locale } = await getDict();
  let stats: { users: number; torrents: number; dead: number; seed_size: number } | null =
    null;
  let latest: Page<TorrentListItem> | null = null;
  try {
    [stats, latest] = await Promise.all([
      api.get<{ users: number; torrents: number; dead: number; seed_size: number }>("/api/v1/stats"),
      paged<TorrentListItem>("/api/v1/torrents", { limit: 10 }),
    ]);
  } catch {
    // 后端未启动时首页降级为空态（本地开发体验）
  }

  return (
    <div className="flex flex-col gap-6">
      {/* 欢迎块（index.php 顶部公告条：奶油底橙描边） */}
      <section className="rounded-[var(--r-md)] border border-[var(--baozi-line)] bg-[linear-gradient(120deg,var(--baozi-cream),#fff8eb)] p-6 shadow-[var(--shadow-card)]">
        <h1 className="font-display text-3xl text-ink">{dict.home.heroTitle}</h1>
        <p className="mt-2 text-sm text-[#6b421f]">{dict.home.heroSubtitle}</p>
      </section>

      {/* 站点统计（旧站首页数据区块口径，票券格子） */}
      {stats && (
        <section className="grid grid-cols-2 gap-3 md:grid-cols-4">
          {[
            { label: dict.home.users, value: stats.users.toLocaleString(dateLocale(locale)) },
            { label: dict.home.torrents, value: stats.torrents.toLocaleString(dateLocale(locale)) },
            { label: dict.home.dead, value: stats.dead.toLocaleString(dateLocale(locale)) },
            {
              label: dict.home.seedSize,
              value: `${(stats.seed_size / 1024 ** 4).toFixed(1)} TB`,
            },
          ].map((s) => (
            <div
              key={s.label}
              className="userstat items-start py-3 text-left"
            >
              <span>{s.label}</span>
              <strong className="num text-xl">{s.value}</strong>
            </div>
          ))}
        </section>
      )}

      {/* 最新种子（table.torrents 复刻） */}
      <section className="flex flex-col gap-3">
        <div className="flex items-baseline justify-between">
          <h2 className="font-display text-2xl text-ink">{dict.home.latest}</h2>
          <Link href="/torrents" className="text-sm">
            {dict.home.viewAll}
          </Link>
        </div>
        {latest && latest.items.length > 0 ? (
          <table className="nexus-table">
            <thead>
              <tr>
                <th className="w-12">{dict.torrents.colType}</th>
                <th>{dict.torrents.colTitle}</th>
                <th className="w-16">{dict.torrents.colComments}</th>
                <th className="w-20">{dict.torrents.colSize}</th>
                <th className="w-16">{dict.torrents.colSeeders}</th>
                <th className="w-16">{dict.torrents.colLeechers}</th>
                <th className="w-16">{dict.torrents.colCompleted}</th>
                <th className="w-28">{dict.torrents.colOwner}</th>
              </tr>
            </thead>
            <tbody>
              {latest.items.slice(0, 10).map((t) => (
                <TorrentTr key={t.id} t={t} />
              ))}
            </tbody>
          </table>
        ) : (
          <div className="flex flex-col items-center gap-2 py-12 text-center">
            <span aria-hidden className="text-[80px] leading-none">
              🥟
            </span>
            <p className="font-display text-lg">{dict.home.empty}</p>
          </div>
        )}
      </section>
    </div>
  );
}
