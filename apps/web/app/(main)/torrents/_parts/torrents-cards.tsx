/**
 * 种子列表的卡片视图与海报墙（方案阶段二：三视图）。
 *
 * 与表格视图共享同一份数据（列表接口已返回 poster/rating/promotion 两列），
 * 纯呈现差异：卡片=左封面右信息（信息密度接近表格，适合浏览）；
 * 海报墙=网格海报（视觉优先，适合找片）。
 * 无 hooks，保持 server component；封面图一律 loading="lazy"。
 */

import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import { formatBytes, promotionBadge } from "@/lib/format";
import { catColor, promoSemanticClass } from "@/components/torrent-table";
import { Icon } from "@/components/icons";
import type { Dict } from "@/i18n/zh-CN";

const clamp2: React.CSSProperties = {
  display: "-webkit-box",
  WebkitLineClamp: 2,
  WebkitBoxOrient: "vertical",
  overflow: "hidden",
};
// —— 样式常量（行宽门禁 ≤80：长 className 一律提到这里，别写在 JSX 里） ——
const CARD_CLS =
  "flex h-full gap-3 rounded-xl border border-line " +
  "bg-[var(--surface-card)] p-3 transition-colors hover:border-sky";
const CARD_TITLE_CLS =
  "text-[13.5px] font-semibold leading-snug text-ink hover:text-sky-deep";
const STATS_CLS =
  "num flex flex-wrap items-center gap-x-3 gap-y-0.5 " +
  "text-[11.5px] text-sub";
const WALL_CLS =
  "grid list-none grid-cols-2 gap-3 p-0 sm:grid-cols-3 " +
  "lg:grid-cols-5 xl:grid-cols-6";
const WALL_ITEM_CLS =
  "group relative block overflow-hidden rounded-xl border border-line " +
  "bg-[var(--surface-card)]";
const WALL_BAR_CLS =
  "absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/78 " +
  "to-transparent px-2 pb-1.5 pt-6";
const WALL_TITLE_CLS =
  "block text-[12px] font-semibold leading-snug text-white";
const WALL_STATS_CLS =
  "num mt-0.5 flex items-center gap-2 text-[10.5px] text-white/80";

/** 封面（外链 poster；无图回退分类色块 + 首字） */
function Cover({
  t,
  className,
  sizes,
}: {
  t: TorrentListItem;
  className: string;
  sizes?: string;
}) {
  if (t.poster) {
    return (
      // eslint-disable-next-line @next/next/no-img-element
      <img
        src={t.poster}
        alt=""
        loading="lazy"
        decoding="async"
        className={`${className} object-cover`}
        style={{ background: catColor(t.category_id) }}
        sizes={sizes}
      />
    );
  }
  return (
    <span
      className={`${className} flex items-center justify-center text-white/85`}
      style={{ background: catColor(t.category_id) }}
      aria-hidden
    >
      {/* 与表格视图同款 fallback：分类色块 + 唱片图标
          （此前用种子名首字符，遇到「【官种】…」这类命名会只显示一个「【」） */}
      <Icon name="disc" size={22} />
    </span>
  );
}

function PromoBadges({ t, dict }: { t: TorrentListItem; dict: Dict }) {
  const promo = promotionBadge(
    (t.promotion as TorrentListItem["promotion"]) ?? null,
  );
  const isNew = Date.now() - new Date(t.created_at).getTime() < 3 * 86400000;
  const userTags = t.tags ?? [];
  // 0170 审核状态徽标：仅待审(0)/被拒(2)行显示（站点开关放行后进列表）
  const approval =
    t.approval_status === 0 || t.approval_status === 2
      ? t.approval_status
      : null;
  if (
    !promo &&
    !isNew &&
    !t.official &&
    approval === null &&
    userTags.length === 0
  )
    return null;
  const promoCls =
    promo && promo.key === "free"
      ? "torrents-promo torrents-promo--free"
      : "torrents-promo";
  return (
    <span className="flex flex-wrap items-center gap-1">
      {promo && (
        <span className={promoCls}>
          {dict.promotion[promo.key]}
        </span>
      )}
      {t.official && (
        <span className="torrents-tag torrents-tag--official">
          {dict.torrents.officialTag}
        </span>
      )}
      {isNew && (
        <span className="torrents-new">{dict.torrents.newTag}</span>
      )}
      {/* 0170 审核状态徽标：与表格行同款（待审琥珀 / 被拒红） */}
      {approval !== null && (
        <span
          className={
            approval === 0
              ? "torrents-approval"
              : "torrents-approval torrents-approval--rejected"
          }
        >
          {approval === 0
            ? dict.torrents.statusPending
            : dict.torrents.statusRejected}
        </span>
      )}
      {/* 用户标签徽标（0159 P1）：与表格行同口径，消费字典样式列 */}
      {userTags.slice(0, 3).map((b) => (
        <span
          key={b.id}
          className="torrents-tag torrents-tag--user"
          style={
            b.bg_color
              ? { background: b.bg_color, color: b.color || undefined }
              : undefined
          }
        >
          {b.name}
        </span>
      ))}
      {userTags.length > 3 && (
        <span
          className="torrents-tag torrents-tag--user torrents-tag--more"
          title={userTags.slice(3).map((b) => b.name).join(" / ")}
        >
          +{userTags.length - 3}
        </span>
      )}
    </span>
  );
}

function Stats({ t, dict }: { t: TorrentListItem; dict: Dict }) {
  const t2 = dict.torrents;
  return (
    <span className={STATS_CLS}>
      <span title={t2.colSeeders}>🌱 {t.seeders}</span>
      <span title={t2.colLeechers}>⬇️ {t.leechers}</span>
      <span title={t2.colCompleted}>✅ {t.times_completed}</span>
      <span title={t2.colSize}>{formatBytes(t.size)}</span>
      <span title={t2.colComments}>💬 {t.comments}</span>
    </span>
  );
}

/** 卡片视图：左封面（2:3，79×104）+ 右信息，一屏 3 列（xl） */
export function TorrentCards({
  items,
  dict,
}: {
  items: TorrentListItem[];
  dict: Dict;
}) {
  return (
    <ul className="grid list-none gap-3 p-0 sm:grid-cols-2 xl:grid-cols-3">
      {items.map((t) => (
        // DOM 契约与表格行同口径（NP pro_* + 断种/官种），脚本三视图通用
        <li
          key={t.id}
          data-torrent-id={t.id}
          className={[
            promoSemanticClass(t.promotion),
            t.seeders === 0 ? "torrent-dead" : undefined,
            t.official ? "torrent-official" : undefined,
          ]
            .filter(Boolean)
            .join(" ") || undefined}
        >
          <article className={CARD_CLS}>
            <Link href={`/torrent/${t.id}`} className="shrink-0">
              <Cover
                t={t}
                className="h-[104px] w-[79px] rounded-lg object-cover"
                sizes="79px"
              />
            </Link>
            <div className="flex min-w-0 flex-1 flex-col gap-1">
              <PromoBadges t={t} dict={dict} />
              <Link
                href={`/torrent/${t.id}`}
                className={CARD_TITLE_CLS}
                style={clamp2}
                title={t.name}
              >
                {t.name}
              </Link>
              {t.small_descr && (
                <span className="truncate text-[11.5px] text-sub">
                  {t.small_descr}
                </span>
              )}
              <span className="mt-auto">
                <Stats t={t} dict={dict} />
              </span>
            </div>
          </article>
        </li>
      ))}
    </ul>
  );
}

/** 海报墙：2/3 网格，移动 2 列 → 桌面 6 列；底部信息条压在图上 */
export function TorrentPosters({
  items,
  dict,
}: {
  items: TorrentListItem[];
  dict: Dict;
}) {
  return (
    <ul className={WALL_CLS}>
      {items.map((t) => (
        <li
          key={t.id}
          data-torrent-id={t.id}
          className={[
            promoSemanticClass(t.promotion),
            t.seeders === 0 ? "torrent-dead" : undefined,
            t.official ? "torrent-official" : undefined,
          ]
            .filter(Boolean)
            .join(" ") || undefined}
        >
          <Link
            href={`/torrent/${t.id}`}
            className={WALL_ITEM_CLS}
            title={t.name}
          >
            <span className="relative block" style={{ aspectRatio: "2 / 3" }}>
              <Cover
                t={t}
                className="absolute inset-0 h-full w-full"
                sizes="(max-width: 640px) 45vw, 220px"
              />
            </span>
            {/* 顶部促销角标 */}
            <span className="absolute left-1.5 top-1.5">
              <PromoBadges t={t} dict={dict} />
            </span>
            {/* 底部信息条 */}
            <span className={WALL_BAR_CLS}>
              <span
                className={WALL_TITLE_CLS}
                style={clamp2}
              >
                {t.name}
              </span>
              <span className={WALL_STATS_CLS}>
                <span>🌱 {t.seeders}</span>
                <span>{formatBytes(t.size)}</span>
              </span>
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
}
