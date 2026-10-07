import { getTopBoards, TopRow } from "@/lib/data";
import { formatBytes, avatarFrameStyle, FrameImageOverlay } from "@/lib/format";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

/** 排行榜（好学站 top.php 口径）：六榜卡片两行 —— 最多魔力/上传量/下载量/最长做种时间/后宫时魔/发种量 */

function medal(rank: number): string {
  if (rank === 1) return "🏆";
  if (rank === 2) return "🥈";
  if (rank === 3) return "🥉";
  return `${rank}`;
}

function fmtDuration(hours: number): string {
  const h = Math.max(0, Math.floor(hours));
  const months = Math.floor(h / 720);
  const days = Math.floor((h % 720) / 24);
  if (months > 0) return `${months} 个月 ${days} 天`;
  if (days > 0) return `${days} 天 ${h % 24} 时`;
  return `${h} 时`;
}

function Avatar({ u }: { u: TopRow }) {
  // 框图优先，否则 CSS 描边；容器 relative 供叠层定位
  const style = u.avatar_frame_image
    ? undefined
    : avatarFrameStyle(u.avatar_frame_css);
  if (u.avatar_url) {
    return (
      <span className="relative inline-flex h-8 w-8 shrink-0" style={style}>
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img
          src={u.avatar_url}
          alt=""
          className="h-8 w-8 rounded-full border border-[var(--baozi-line)] object-cover"
        />
        <FrameImageOverlay url={u.avatar_frame_image} />
      </span>
    );
  }
  return (
    <span
      className="relative flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-[var(--baozi-orange)]/15 text-sm font-black text-[var(--baozi-orange-dark)]"
      style={style}
    >
      {u.username.slice(0, 1).toUpperCase()}
      <FrameImageOverlay url={u.avatar_frame_image} />
    </span>
  );
}

/** 榜单数值列的表头单位。
 *  ZT2（2026-10-03）：原实现按**本地化标题**猜单位（`title.includes("做种"/"后宫"/
 *  "魔力")`）——标题一翻译（en/zh-TW/ja）所有分支即失效，列头永远退化成「数量」。
 *  改为由调用方按后端的**稳定 board key**显式传入，与语言无关。 */
type BoardUnit = "size" | "hours" | "perHour" | "magic" | "count";

function valueHeader(unit: BoardUnit, currency: string): string {
  if (unit === "size") return "大小";
  if (unit === "hours") return "时长";
  if (unit === "perHour") return "每小时";
  if (unit === "magic") return currency;
  return "数量";
}

function Board({
  icon,
  title,
  rows,
  empty,
  fmt,
  unit,
  currency,
}: {
  icon: string;
  title: string;
  rows: TopRow[];
  empty: string;
  fmt: (v: number) => string;
  unit: BoardUnit;
  currency: string;
}) {
  return (
    <section className="baozi-panel overflow-hidden">
      <header className="baozi-panel__head justify-center">
        <h2>
          <span aria-hidden="true">{icon}</span> {title}
        </h2>
      </header>
      {rows.length === 0 ? (
        <p className="funbox__empty">{empty}</p>
      ) : (
        <table className="w-full border-collapse">
          <thead>
            <tr className="border-b border-[var(--border-soft)] text-[12px] text-[var(--text-faint)]">
              <th className="w-12 py-2 pl-3 text-left font-bold">排名</th>
              <th className="py-2 text-left font-bold">用户</th>
              <th className="py-2 pr-3 text-right font-bold">
                {valueHeader(unit, currency)}
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((u) => (
              <tr
                key={`${u.rank}-${u.username}`}
                className={`border-b border-dashed border-[var(--border-soft)] last:border-0 ${
                  u.rank === 1
                    ? "bg-[var(--promo-free-bg)]/60"
                    : u.rank === 2
                      ? "bg-[var(--baozi-cream)]"
                      : u.rank === 3
                        ? "bg-[var(--promo-x2-bg)]/40"
                        : ""
                }`}
              >
                <td className="py-2 pl-3 text-sm font-bold text-[var(--text-faint)]">
                  {medal(u.rank)}
                </td>
                <td className="min-w-0 py-2">
                  <div className="flex items-center gap-2">
                    <Avatar u={u} />
                    <div className="min-w-0">
                      <a
                        href={`/users?q=${encodeURIComponent(u.username)}`}
                        className="block max-w-[160px] truncate text-sm font-bold text-ink hover:text-[var(--baozi-orange)]"
                      >
                        {u.username}
                      </a>
                      <span className="block max-w-[160px] truncate text-[12px] text-[var(--text-faint)]">
                        {u.title || u.class_name}
                      </span>
                    </div>
                  </div>
                </td>
                <td className="py-2 pr-3 text-right text-sm font-bold text-ink">
                  {fmt(u.val)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

export default async function TopPage() {
  const { dict, currency } = await getDict();
  const t = dict.top;
  const b = await getTopBoards();

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Top Charts</div>
          <h1 className="font-display text-2xl">{t.title}</h1>
        </div>
      </div>
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <Board
          currency={currency}
          icon="💰"
          title={t.boardBonus.replace("{magic}", currency)}
          rows={b.bonus}
          empty={t.empty}
          unit="magic"
          fmt={(v) => Math.round(v).toLocaleString("zh-CN")}
        />
        <Board
          currency={currency}
          icon="⬆️"
          title={t.boardUploaded}
          rows={b.uploaded}
          empty={t.empty}
          unit="size"
          fmt={formatBytes}
        />
        <Board
          currency={currency}
          icon="⬇️"
          title={t.boardDownloaded}
          rows={b.downloaded}
          empty={t.empty}
          unit="size"
          fmt={formatBytes}
        />
        <Board
          currency={currency}
          icon="⏳"
          title={t.boardSeedtime}
          rows={b.seedtime}
          empty={t.empty}
          unit="hours"
          fmt={fmtDuration}
        />
        <Board
          currency={currency}
          icon="💒"
          title={t.boardHourly}
          rows={b.hourly}
          empty={t.empty}
          unit="perHour"
          fmt={(v) => v.toFixed(2)}
        />
        <Board
          currency={currency}
          icon="🌱"
          title={t.boardTorrents}
          rows={b.torrents}
          empty={t.empty}
          unit="count"
          fmt={(v) => Math.round(v).toLocaleString("zh-CN")}
        />
      </div>
    </div>
  );
}
