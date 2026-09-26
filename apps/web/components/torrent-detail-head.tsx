import { DownloadButton } from "@/components/download-button";
import { TorrentSocial } from "@/components/torrent-social";
import { ResurrectButton } from "@/components/resurrect-button";
import { WishlistButton } from "@/components/wishlist";
import { PosterBlock } from "@/components/torrent-detail-parts";
import { promotionBadge } from "@/lib/format";
import { dateLocale, type Locale } from "@/i18n/config";
import type { Dict } from "@/i18n/server";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { TorrentDetailExt } from "@/components/torrent-detail-blocks";

/** 种子详情页海报头（馒头/阳光口径：左海报 + 右主信息 + 操作行 + 状态卡）。
 *  从 app/(main)/torrent/[id]/page.tsx 按域拆出；数据装载与派生值留在 page.tsx。
 *  0173 排版整理：标签行入头部（副标题↓、发布人↑）；操作区纵向语义分行
 *  （主操作 → 互动与打赏 → 管理），不再混排换行。 */

export function TorrentHead({
  t,
  ext,
  dict,
  locale,
  left,
  category,
  categoryColor,
  subtitleChain,
  relTime,
  tags,
  manage,
}: {
  t: TorrentListItem;
  ext: TorrentDetailExt | null;
  dict: Dict;
  locale: Locale;
  /** 促销剩余时间文案（好学站「x天x时」口径），null = 无促销 */
  left: string | null;
  /** 分类显示名：由 page 侧按站点档案 categories 解析（category_id 的外键目标），
   *  组件内不再持有任何站型词表 */
  category: string;
  /** 分类色：page 侧按 categories.bg_color 解析 */
  categoryColor: string;
  /** 副题链（学段 · 媒介 · 版本） */
  subtitleChain: string;
  /** 相对时间（馒头口径：x 天前；完整时间放 title） */
  relTime: (iso: string) => string;
  /** 标签行（0173：副标题与发布人之间；undefined = 不渲染） */
  tags?: React.ReactNode;
  /** 管理操作（编辑/补种/删除/促销购）：渲染进头部管理行 */
  manage?: React.ReactNode;
}) {
  const d = dict.tdetail;
  const promo = promotionBadge(t.promotion);
  return (
    <section className="td-head nexus-detail">
      <PosterBlock
        category={category}
        color={categoryColor}
        poster={t.poster}
      />

      <div className="td-head__main">
        <h1 className="td-head__name font-display">{t.name}</h1>
        {(t.small_descr || subtitleChain) && (
          <p className="td-head__sub">
            {subtitleChain && (
              <span className="td-head__chain">{subtitleChain}</span>
            )}
            {subtitleChain && t.small_descr && <span aria-hidden> · </span>}
            {t.small_descr}
          </p>
        )}

        {/* 标签行（0173：副标题与发布人之间） */}
        {tags && <div className="td-head__tags">{tags}</div>}

        {/* H&R 义务明示（评审 P1-5）：下载前必须可见，否则用户被追责时无预期。
         *  hr_policy.enabled=false → 免 H&R；null/无键 → 站点默认（与 worker
         *  jobs/hr.rs 的 COALESCE 口径一致：days=14 / seed_hours=48）。 */}
        {(() => {
          const hr = t.hr_policy;
          const on = hr ? hr.enabled !== false : true;
          const days = hr?.days ?? 14;
          const hours = hr?.seed_hours ?? 48;
          return (
            <p
              className={`td-head__hr ${
                on ? "td-head__hr--on" : "td-head__hr--off"
              }`}
            >
              {on
                ? d?.hrOn
                    ?.replace("{days}", `${days}`)
                    .replace("{hours}", `${hours}`)
                : d?.hrOff}
            </p>
          );
        })()}

        <div className="td-head__badges">
          {t.sticky && (
            <span className="sticker bg-sun text-ink">
              {dict.torrent.sticky}
            </span>
          )}
          {promo && (
            <span className={`sticker ${promo.className}`}>
              {dict.promotion[promo.key]}
            </span>
          )}
          {left && (
            <span className="td-head__left">
              {d?.promoLeft} {left}
            </span>
          )}
          {t.official && (
            <span className="sticker bg-indigo text-white">
              {dict.torrent.official}
            </span>
          )}
          {t.anonymous && (
            <span className="sticker td-anon-sticker">
              {dict.torrent.anonymous}
            </span>
          )}
        </div>

        <div className="td-head__meta">
          <span>
            {dict.torrent.uploader}：
            <b>
              {t.anonymous ? dict.torrent.anonymous : (t.owner_name ?? "—")}
            </b>
          </span>
          <span
            title={new Date(t.created_at).toLocaleString(dateLocale(locale))}
          >
            {dict.torrent.uploadedAt}：<b>{relTime(t.created_at)}</b>
          </span>
          <span
            title={new Date(ext?.last_action ?? t.created_at).toLocaleString(
              dateLocale(locale),
            )}
          >
            {d?.lastActivity}：
            <b>{relTime(ext?.last_action ?? t.created_at)}</b>
          </span>
        </div>

        {/* 操作区（0173 语义分行）：主操作 → 互动与打赏 → 管理 */}
        <div className="td-head__actions">
          <div className="td-head__actions-row">
            <DownloadButton
              torrentId={t.id}
              name={t.name}
              price={ext?.price}
              purchased={ext?.purchased}
              isOwner={ext?.is_owner}
            />
            <WishlistButton keyword={t.name} />
          </div>
          <TorrentSocial torrentId={t.id} />
          <div className="td-head__actions-row">
            {t.seeders === 0 && (
              <ResurrectButton torrentId={t.id} name={t.name} />
            )}
            {manage}
          </div>
        </div>
      </div>

      {/* 状态卡（阳光站口径：做种/下载/完成三色统计） */}
      <aside className="td-stats">
        <div className="td-stats__row td-stats__row--seed">
          <span>{dict.torrent.seeding}</span>
          <b className="num">{t.seeders}</b>
        </div>
        <div className="td-stats__row td-stats__row--leech">
          <span>{dict.torrent.leeching}</span>
          <b className="num">{t.leechers}</b>
        </div>
        <div className="td-stats__row td-stats__row--done">
          <span>{dict.torrent.completed}</span>
          <b className="num">{t.times_completed}</b>
        </div>
        <div className="td-stats__row">
          <span>{d?.thanksCountLabel ?? dict.torrent.thanksCount}</span>
          <b className="num">{ext?.thanks_count ?? 0}</b>
        </div>
        <div className="td-stats__row">
          <span>{d?.heatViews}</span>
          <b className="num">{ext?.views ?? "—"}</b>
        </div>
        {/* 健康度（NP 口径：做种=0 视为死种） */}
        <p
          className={`td-stats__health ${
            t.seeders === 0
              ? "td-stats__health--dead"
              : "td-stats__health--alive"
          }`}
        >
          {t.seeders === 0 ? d?.healthDead : d?.healthAlive}
        </p>
      </aside>
    </section>
  );
}
