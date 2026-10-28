import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import Link from "next/link";
import { formatBytes } from "@/lib/format";

/** 长类名抽常量守行宽门禁 */
const THEAD_ROW =
  "border-b border-[var(--border-soft)] text-[12px] text-[var(--text-faint)]";
const ROW_BASE =
  "border-b border-dashed border-[var(--border-soft)] last:border-0";
const ROW_TOP = `${ROW_BASE} bg-[var(--promo-free-bg)]/60`;
const ROW_LINE = ROW_BASE;
const TD_RANK = "py-2 pl-3 text-sm font-bold text-[var(--text-faint)]";
const LINK_NAME =
  "block max-w-[280px] truncate text-sm font-bold " +
  "text-ink hover:text-[var(--brand-orange)]";

/** 内容热榜三件套（0424 P1-1）：today=今日完成增量 / week=近7天完成增量
 *  / free=当前免费中按做种数。此前全站只有用户六榜，没有任何内容侧
 *  榜单——「热门榜 + 免费区」是 PT 站拉下载的第一入口。 */

export interface HotTorrentRow {
  id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  val: number;
  rating: string | null;
  poster: string | null;
  official: boolean;
}

export interface TopTorrents {
  today: HotTorrentRow[];
  week: HotTorrentRow[];
  free: HotTorrentRow[];
}

function medal(rank: number): string {
  if (rank === 1) return "🔥";
  if (rank === 2) return "🥈";
  if (rank === 3) return "🥉";
  return `${rank}`;
}

function HotBoard({
  icon,
  title,
  rows,
  empty,
  valLabel,
  moreHref,
  colTorrent,
}: {
  icon: string;
  title: string;
  rows: HotTorrentRow[];
  empty: string;
  valLabel: string;
  moreHref: string;
  colTorrent: string;
}) {
  return (
    <section className="brand-panel overflow-hidden">
      <header className="brand-panel__head justify-between">
        <h2>
          <span aria-hidden="true">{icon}</span> {title}
        </h2>
        <Link href={moreHref} className="text-xs">
          →
        </Link>
      </header>
      {rows.length === 0 ? (
        <p className="funbox__empty">{empty}</p>
      ) : (
        <table className="w-full border-collapse">
          <thead>
            <tr
              className={THEAD_ROW}
            >
              <th className="w-10 py-2 pl-3 text-left font-bold">#</th>
              <th className="py-2 text-left font-bold">
                {colTorrent}
              </th>
              <th className="w-20 py-2 pr-3 text-right font-bold">
                {valLabel}
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((t, i) => (
              <tr
                key={t.id}
                className={i === 0 ? ROW_TOP : ROW_LINE}
              >
                <td className={TD_RANK}>
                  {medal(i + 1)}
                </td>
                <td className="min-w-0 py-2">
                  <Link
                    href={`/torrent/${t.id}`}
                    className={LINK_NAME}
                    title={t.name}
                  >
                    {t.official ? "🎓 " : ""}
                    {t.name}
                  </Link>
                  <span className="block text-[12px] text-[var(--text-faint)]">
                    {formatBytes(t.size)} · 👥 {t.seeders}
                  </span>
                </td>
                <td className="py-2 pr-3 text-right text-sm font-bold text-ink">
                  {t.val}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

/** 服务端取数 + 渲染三榜（RSC；与 /top 用户六榜同一页面域） */
export async function TopTorrentBoards() {
  const { dict } = await getDict();
  const t = dict.topTorrents;
  let d: TopTorrents | null = null;
  try {
    d = await api.get<TopTorrents>("/api/v1/top/torrents");
  } catch {
    d = null;
  }
  if (!d) return null;
  const allEmpty = [d.today, d.week, d.free].every((r) => r.length === 0);
  if (allEmpty) return null;
  return (
    <div className="grid gap-4 md:grid-cols-3">
      <HotBoard
        icon="🔥"
        title={t.today}
        rows={d.today}
        empty={t.empty}
        valLabel={t.unitDone}
        moreHref="/torrents?sort=created&order=desc"
        colTorrent={t.colTorrent}
      />
      <HotBoard
        icon="📦"
        title={t.week}
        rows={d.week}
        empty={t.emptyWeek}
        valLabel={t.unitDone}
        moreHref="/torrents"
        colTorrent={t.colTorrent}
      />
      <HotBoard
        icon="🎁"
        title={t.free}
        rows={d.free}
        empty={t.emptyFree}
        valLabel={t.unitSeeders}
        moreHref="/torrents?promo=free"
        colTorrent={t.colTorrent}
      />
    </div>
  );
}
