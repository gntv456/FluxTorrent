import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import { formatBytes, promotionBadge } from "@/lib/format";
import { getDict } from "@/i18n/server";
import {
  catColor,
  getTorrentDicts,
} from "@/lib/site-profile";
import { Icon } from "@/components/icons";
import { dateLocale } from "@/i18n/config";

/**
 * 种子行卡片（设计稿 B3 桌面 .tor-card / C2 移动 .m-tor 的统一组件）。
 * 硬性规则（§7.6-3）：三端三形态字段顺序一致。
 */
export async function TorrentRow({ t }: { t: TorrentListItem }) {
  const { dict, locale } = await getDict();
  const dicts = await getTorrentDicts();
  const promo = promotionBadge(t.promotion);
  return (
    <Link
      href={`/torrent/${t.id}`}
      className="flex items-center gap-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3 shadow-[var(--shadow-card)] transition-transform hover:-translate-y-0.5 active:scale-[0.99]"
    >
      {/* 左：分类色块（34px 圆角 9px） */}
      <span
        aria-hidden
        className="h-[34px] w-[34px] shrink-0 rounded-[9px]"
        style={{ background: catColor(dicts.colors, t.category_id) }}
      />
      {/* 中：主区 */}
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5">
          {t.sticky && (
            <span className="sticker bg-sun text-ink">
              {dict.torrent.sticky}
            </span>
          )}
          <span className="truncate font-bold text-ink">{t.name}</span>
          {promo && (
            <span className={`sticker ${promo.className}`}>
              {dict.promotion[promo.key]}
            </span>
          )}
          {t.official && (
            <span className="sticker bg-indigo text-white">
              {dict.torrent.official}
            </span>
          )}
        </div>
        {t.small_descr && (
          <p className="mt-0.5 truncate text-xs text-sub">{t.small_descr}</p>
        )}
        {/* 维度链（R3-三步）：sections 单源——站型任意维度名拼接 */}
        {(t.sec_names?.length ?? 0) > 0 && (
          <p className="mt-0.5 flex flex-wrap gap-1.5 text-[11px] text-sub">
            {(t.sec_names ?? []).slice(0, 3).map((n, i) => (
              <span key={i}>{n}</span>
            ))}
          </p>
        )}
        <p className="mt-0.5 text-[11px] text-sub">
          {t.anonymous ? dict.torrent.anonymous : (t.owner_name ?? "—")} ·{" "}
          {new Date(t.created_at).toLocaleDateString(dateLocale(locale))}
        </p>
      </div>
      {/* 右：数据列（tabular-nums；做种绿/下载橙/完成灰） */}
      <div className="num flex shrink-0 flex-col items-end gap-0.5 text-xs">
        <span className="text-sub">{formatBytes(t.size)}</span>
        <span className="text-mint">
          {dict.torrent.seeding} {t.seeders}
        </span>
        <span className="text-coral">
          {dict.torrent.leeching} {t.leechers}
        </span>
        <span className="text-sub">
          {dict.torrent.completed} {t.times_completed}
        </span>
      </div>
    </Link>
  );
}

/** 空态（设计稿：吉祥物 80px + 蜡笔字标题 + 行动按钮）。
 *  aliveFiltered：当前列表按「仅活种」口径过滤（默认视图）——空了不等于站里没种子，
 *  可能只是全部断种；文案改写并给出切「全部」的入口，否则与侧栏「发布种子 N」矛盾
 *  （评审 P0-2：同屏「共 0 个种子」+「发布种子 1」的口径打架）。 */
export async function EmptyTorrents({
  aliveFiltered = false,
}: {
  aliveFiltered?: boolean;
}) {
  const { dict } = await getDict();
  return (
    <div
      className="torrents-empty flex flex-col items-center gap-3 py-16
        text-center"
    >
      <Icon name="seed" size={72} className="text-[var(--sky)]" />
      <h2 className="font-display text-xl text-ink">
        {aliveFiltered
          ? dict.torrent.emptyAliveTitle
          : dict.torrent.emptyTitle}
      </h2>
      {aliveFiltered ? (
        <a
          href="/torrents?alive=0"
          className="rounded-full border border-line px-5 py-2 text-sm font-bold text-sky-deep hover:bg-[var(--surface-card)]"
        >
          {dict.torrent.emptyAliveAction}
        </a>
      ) : (
        <Link
          href="/upload"
          className="rounded-full bg-coral px-5 py-2 text-sm font-bold text-white"
        >
          {dict.common.publish}
        </Link>
      )}
    </div>
  );
}
