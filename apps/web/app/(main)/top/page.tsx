import { getTopBoards, TopRow } from "@/lib/data";
import { formatBytes } from "@/lib/format";
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
  if (u.avatar_url) {
    return (
      // eslint-disable-next-line @next/next/no-img-element
      <img
        src={u.avatar_url}
        alt=""
        className="h-8 w-8 shrink-0 rounded-full border border-[var(--baozi-line)] object-cover"
      />
    );
  }
  return (
    <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-[var(--baozi-orange)]/15 text-sm font-black text-[var(--baozi-orange-dark)]">
      {u.username.slice(0, 1).toUpperCase()}
    </span>
  );
}

function valueHeader(title: string, empty: string, isBytes: boolean): string {
  if (isBytes) return "大小";
  if (title.includes("做种")) return "时长";
  if (title.includes("后宫")) return "每小时";
  if (title.includes("魔力")) return "魔力";
  return "数量";
}

function Board({
  icon,
  title,
  rows,
  empty,
  fmt,
  isBytes = false,
}: {
  icon: string;
  title: string;
  rows: TopRow[];
  empty: string;
  fmt: (v: number) => string;
  isBytes?: boolean;
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
            <tr className="border-b border-[var(--border-soft)] text-[11px] text-[var(--text-faint)]">
              <th className="w-12 py-2 pl-3 text-left font-bold">排名</th>
              <th className="py-2 text-left font-bold">用户</th>
              <th className="py-2 pr-3 text-right font-bold">{valueHeader(title, empty, isBytes)}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((u) => (
              <tr
                key={`${u.rank}-${u.username}`}
                className={`border-b border-dashed border-[var(--border-soft)] last:border-0 ${
                  u.rank === 1 ? "bg-[var(--promo-free-bg)]/60" : u.rank === 2 ? "bg-[var(--baozi-cream)]" : u.rank === 3 ? "bg-[var(--promo-x2-bg)]/40" : ""
                }`}
              >
                <td className="py-2 pl-3 text-sm font-bold text-[var(--text-faint)]">{medal(u.rank)}</td>
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
                      <span className="block max-w-[160px] truncate text-[11px] text-[var(--text-faint)]">
                        {u.title || u.class_name}
                      </span>
                    </div>
                  </div>
                </td>
                <td className="py-2 pr-3 text-right text-sm font-bold text-ink">{fmt(u.val)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

export default async function TopPage() {
  const { dict } = await getDict();
  const t = dict.top;
  const b = await getTopBoards();

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{t.title}</h1>
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <Board icon="💰" title={t.boardBonus} rows={b.bonus} empty={t.empty} fmt={(v) => Math.round(v).toLocaleString("zh-CN")} />
        <Board icon="⬆️" title={t.boardUploaded} rows={b.uploaded} empty={t.empty} fmt={formatBytes} isBytes />
        <Board icon="⬇️" title={t.boardDownloaded} rows={b.downloaded} empty={t.empty} fmt={formatBytes} isBytes />
        <Board icon="⏳" title={t.boardSeedtime} rows={b.seedtime} empty={t.empty} fmt={fmtDuration} />
        <Board icon="💒" title={t.boardHourly} rows={b.hourly} empty={t.empty} fmt={(v) => v.toFixed(2)} />
        <Board icon="🌱" title={t.boardTorrents} rows={b.torrents} empty={t.empty} fmt={(v) => Math.round(v).toLocaleString("zh-CN")} />
      </div>
    </div>
  );
}
