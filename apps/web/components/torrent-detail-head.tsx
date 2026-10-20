import { DownloadButton, MagnetButton } from "@/components/download-button";
import { HeadActionsMore } from "@/components/torrent-head-more";
import { ThankButton, TorrentSocial } from "@/components/torrent-social";
import { ResurrectButton } from "@/components/resurrect-button";
import { TrumpButton } from "@/components/torrent-trump-button";
import { WishlistButton } from "@/components/wishlist";
import { PosterBlock } from "@/components/torrent-detail-parts";
import { promotionBadge } from "@/lib/format";
import { dateLocale, type Locale } from "@/i18n/config";
import type { Dict } from "@/i18n/server";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { TorrentDetailExt } from "@/components/torrent-detail-blocks";
import type { RipLog } from "@/components/torrent-rip-logs";
import { Badge } from "@/components/ui/badge";

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
  ripLogs,
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
  /** 抓轨日志行（0312）：徽标取多碟**最低**分；无日志行不出徽标 */
  ripLogs?: RipLog[];
  /** 管理操作（编辑/补种/删除/促销购）：渲染进头部管理行 */
  manage?: React.ReactNode;
}) {
  const d = dict.tdetail;
  const promo = promotionBadge(t.promotion);
  // 日志分徽标（0312）：取**有效分**（0337 改判优先）后多碟取最低，与面板同口径
  const ripScores = (ripLogs ?? [])
    .map((l) => l.adjusted_score ?? l.log_score)
    .filter((v): v is number => v !== null);
  const ripScore = ripScores.length > 0 ? Math.min(...ripScores) : null;
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

        {/* H&R 义务明示（评审 P1-5）：下载前必须可见，否则被追责时无预期。
         *  这里的 `hr_policy` 已由详情/聚合出口换成**本人视角的生效义务**
         *  （种子 policy → 站点缺省 hr_days/hr_hours → 档位折扣，算在
         *  `repo_hr::effective`，与 worker 建快照同一句 SQL）。
         *  前端不许再自算缺省：实测 strict 档（现网绑 class 1 普通会员）
         *  被执法成 72 小时／11 天，照旧文案「48 小时」攒够照样吃违规，
         *  累计到阈值直接停下载权。`?? 14/48` 只作后端未下发时的兜底。 */}
        {(() => {
          const hr = t.hr_policy;
          const on = hr?.enabled !== false;
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
            <Badge tone="warn">
              {dict.torrent.sticky}
            </Badge>
          )}
          {promo && (
            <Badge tone={promo.tone} variant={promo.variant}>
              {dict.promotion[promo.key]}
            </Badge>
          )}
          {left && (
            <span className="td-head__left">
              {d?.promoLeft} {left}
            </span>
          )}
          {t.official && (
            <Badge tone="brand" variant="solid">
              {dict.torrent.official}
            </Badge>
          )}
          {ripScore !== null && (
            <Badge
              tone={ripScore >= 100 ? "warn" : "brand"}
              title={d.ripLogs}
            >
              {d.ripBadge.replace("{n}", `${ripScore}`)}
            </Badge>
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

        {/* 操作区（本地完善版样图：行 1 = 下载种子 + 心愿单；行 2 =
            感谢发布者 + 更多操作。磁力/收藏/评论/打赏/抢种/复活/管理
            全部收进「更多操作」面板——桌面不再内联铺开） */}
        <div className="td-head__actions">
          <div className="td-head__actions-row">
            <DownloadButton
              torrentId={t.id}
              name={t.name}
              price={ext?.price}
              purchased={ext?.purchased}
              isOwner={ext?.is_owner}
              showMagnet={false}
            />
            <WishlistButton keyword={t.name} />
          </div>
          <div className="td-head__actions-row">
            <ThankButton
              torrentId={t.id}
              initialThanked={ext?.thanked ?? false}
            />
            <HeadActionsMore label={dict.torrents.moreActions}>
              <MagnetButton torrentId={t.id} />
              {/* 「感谢发布者」第二位置：<768 头部整块隐藏，面板内补一枚
                  （两个实例按断点互斥显示，见 base.css .td-thank--sheet） */}
              <span className="td-thank--sheet">
                <ThankButton
                  torrentId={t.id}
                  initialThanked={ext?.thanked ?? false}
                />
              </span>
              <TorrentSocial
                torrentId={t.id}
                initialBookmarked={ext?.bookmarked ?? false}
                initialThanked={ext?.thanked ?? false}
              />
              <div className="td-head__actions-row">
                {/* 后端明确拒绝领取自己的种（resurrections.rs:76）——
                    种主看到这枚按钮点下去必回 400，直接不渲染 */}
                {t.seeders === 0 && !ext?.is_owner && (
                  <ResurrectButton torrentId={t.id} name={t.name} />
                )}
                <TrumpButton torrentId={t.id} />
                {manage}
              </div>
            </HeadActionsMore>
          </div>
        </div>
      </div>
    </section>
  );
}
