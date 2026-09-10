import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import {
  editionName,
  formatBytes,
  promotionBadge,
} from "@/lib/format";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";

/** 包子站分类色（奶面小方块：类型列） */
const CAT_COLORS: Record<number, string> = {
  1: "#f6a5c0",
  2: "#7fb7e6",
  3: "#8fd6b5",
  4: "#f4d06f",
  5: "#b5a6f0",
  6: "#f6a07a",
  7: "#c9b8a3",
};

function catColor(id: number): string {
  return CAT_COLORS[id] ?? "#c9b8a3";
}

/**
 * 种子表行（包子站 table.torrents 行复刻）：
 * 类型色块 · 标题(置顶/促销/官种徽) + 副题 · 存活时间。
 */
async function TorrentTr({ t }: { t: TorrentListItem }) {
  const { dict, locale } = await getDict();
  const promo = promotionBadge(t.promotion);
  const edition = editionName(t.edition_id);
  const grade =
    t.grade_id !== null ? dict.torrents.grades[t.grade_id + 1] : undefined;
  const age = Date.now() - new Date(t.created_at).getTime();
  const ageDays = Math.floor(age / 86400000);
  const alive =
    ageDays < 1
      ? dict.torrents.today
      : ageDays < 30
        ? fmtDays(dict, ageDays)
        : `${Math.floor(ageDays / 30)} ${dict.torrents.months}`;
  return (
    <tr>
      <td>
        <span
          aria-hidden
          className="inline-block h-6 w-6 rounded-[6px] border border-[var(--baozi-line-soft)]"
          style={{ background: catColor(t.category_id) }}
          title={dict.torrents.categories[t.category_id] ?? ""}
        />
      </td>
      <td>
        <div className="flex flex-wrap items-center gap-1.5">
          {t.sticky && (
            <span className="sticker bg-[var(--baozi-orange)] text-white">
              {dict.torrent.sticky}
            </span>
          )}
          {t.official && (
            <span className="sticker">{dict.torrent.official}</span>
          )}
          {promo && <span className="sticker">{dict.promotion[promo.key]}</span>}
          <Link href={`/torrent/${t.id}`} className="font-bold">
            {t.name}
          </Link>
        </div>
        {(t.small_descr || grade || edition) && (
          <p className="mt-0.5 flex flex-wrap gap-1.5 text-[11px] text-sub">
            {grade && <span>{grade}</span>}
            {grade && edition && <span aria-hidden>·</span>}
            {edition && <span>{edition}</span>}
            {t.small_descr && <span>{t.small_descr}</span>}
          </p>
        )}
        <p className="mt-0.5 text-[11px] text-sub">
          {dict.torrents.alive}: {alive}
        </p>
      </td>
      <td className="num">{t.comments}</td>
      <td className="num">{formatBytes(t.size)}</td>
      <td className="num seed-arrow">{t.seeders}</td>
      <td className="num leech-arrow">{t.leechers}</td>
      <td className="num">{t.times_completed}</td>
      <td>
        {t.anonymous ? (
          <span className="text-sub">{dict.torrent.anonymous}</span>
        ) : (
          <span className="font-semibold text-[var(--baozi-orange-dark)]">
            {t.owner_name ?? "—"}
          </span>
        )}
        <p className="text-[11px] text-sub">
          {new Date(t.created_at).toLocaleDateString(dateLocale(locale))}
        </p>
      </td>
    </tr>
  );
}

function fmtDays(dict: Awaited<ReturnType<typeof getDict>>["dict"], n: number) {
  return `${n} ${dict.torrents.days}`;
}

export { TorrentTr, catColor };
