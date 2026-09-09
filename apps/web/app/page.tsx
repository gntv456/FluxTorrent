import Link from "next/link";
import { api, paged } from "@/lib/api-client";
import { TorrentRow } from "@/components/torrent";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import type { Page, TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

/** 首页工作台（设计稿：站点统计 + 最新种子） */
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
      {/* Hero 渐变（§7.1: 135deg sky→indigo→candy） */}
      <section className="rounded-[var(--r-xl)] bg-[linear-gradient(135deg,var(--sky),var(--indigo),var(--candy))] p-8 text-white">
        <h1 className="font-display text-4xl">{dict.home.heroTitle}</h1>
        <p className="mt-2 text-white/85">{dict.home.heroSubtitle}</p>
        {/* 彩带进度条（§3.2 品牌元素） */}
        {stats && (
          <div className="mt-4 h-2 w-full max-w-md rounded-full bg-[var(--ribbon)]" />
        )}
      </section>

      {/* 站点统计（旧站首页数据区块口径） */}
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
              className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
            >
              <p className="text-xs text-sub">{s.label}</p>
              <p className="num mt-1 text-xl">{s.value}</p>
            </div>
          ))}
        </section>
      )}

      {/* 最新种子 */}
      <section className="flex flex-col gap-3">
        <div className="flex items-baseline justify-between">
          <h2 className="font-display text-2xl">{dict.home.latest}</h2>
          <Link href="/torrents" className="text-sm text-sky">
            {dict.home.viewAll}
          </Link>
        </div>
        {latest && latest.items.length > 0 ? (
          <div className="flex flex-col gap-2">
            {latest.items.slice(0, 10).map((t) => (
              <TorrentRow key={t.id} t={t} />
            ))}
          </div>
        ) : (
          <div className="flex flex-col items-center gap-2 py-12 text-center">
            <span aria-hidden className="text-[80px] leading-none">
              🦉
            </span>
            <p className="font-display text-lg">{dict.home.empty}</p>
          </div>
        )}
      </section>
    </div>
  );
}
