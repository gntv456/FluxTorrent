import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { PreserveItem } from "@/lib/data";
import { editionName, formatBytes, promotionBadge } from "@/lib/format";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { TorrentActions } from "@/components/torrent-actions";

/** 保种区行（/preserve 下发的同构行：id 键为 torrent_id） */
type PreserveRowAlias = PreserveItem;

/** 分类色（好学站 catsprites 色系）：类型列色块 + 无封面时的回退底色 */
const CAT_COLORS: Record<number, string> = {
  1: "#f6a5c0",
  2: "#7fb7e6",
  3: "#8fd6b5",
  4: "#f4d06f",
  5: "#b5a6f0",
  6: "#f6a07a",
  7: "#c9b8a3",
};

/** 促销行高亮（好学站口径：免费=浅蓝 #89c9e6 系、2x=暖黄系） */
function promoRowBg(promo: string | null | undefined): string | undefined {
  if (promo === "free" || promo === "x2free") return "var(--promo-free-bg)";
  if (promo === "x2" || promo === "x2half") return "var(--promo-x2-bg)";
  return undefined;
}

function catColor(id: number): string {
  return CAT_COLORS[id] ?? "#c9b8a3";
}

/** 剩余时间（好学站「6天23时」口径；无截止则不显示） */
function remaining(end: string | null | undefined, now: number): string | null {
  if (!end) return null;
  const ms = new Date(end).getTime() - now;
  if (ms <= 0) return null;
  const d = Math.floor(ms / 86400000);
  const h = Math.floor((ms % 86400000) / 3600000);
  return `${d}天${h}时`;
}

/**
 * 种子表行（好学站 torrents.php 行结构复刻）：
 * 类型色块 | 封面 46px（media_info.poster 外链，无图回退类型色块）
 * | 标题三行（主标题[置顶/新] → 促销状态+剩余时间紧跟种子名 → 副题链 → 标签·发布者）
 * | 评论/存活/大小/做种/下载/完成 | 行为（下载 + ⋮ 下拉：收藏/编辑/删除）。
 * id 字段：资源库行是 t.id，保种区行是 t.torrent_id（adaptId 兼容两种来源）。
 */
async function TorrentTr({
  t,
  extra,
}: {
  t: TorrentListItem | PreserveRowAlias;
  /** 行尾附加列（保种区的认领人/认领按钮）；渲染在数字列后、行为列前 */
  extra?: React.ReactNode;
}) {
  const { dict, locale } = await getDict();
  const id = "torrent_id" in t ? t.torrent_id : t.id;
  const promo = promotionBadge(
    (t.promotion as TorrentListItem["promotion"]) ?? null,
  );
  const edition = editionName(t.edition_id);
  const grade =
    t.grade_id !== null ? dict.torrents.grades[t.grade_id + 1] : undefined;
  const now = Date.now();
  const age = now - new Date(t.created_at).getTime();
  const ageDays = Math.floor(age / 86400000);
  const alive =
    ageDays < 1
      ? dict.torrents.today
      : ageDays < 30
        ? `${ageDays} ${dict.torrents.days}`
        : `${Math.floor(ageDays / 30)} ${dict.torrents.months}`;
  const isNew = age < 3 * 86400000; // 「新」标：3 天内（NP new 标同数量级）
  const left = remaining(t.promotion_ends_at, now);
  const rowBg = promoRowBg(t.promotion);
  const subtitle = t.small_descr || "";

  return (
    <tr style={rowBg ? { background: rowBg } : undefined}>
      {/* 类型 */}
      <td className="torrents-td-cat">
        <span
          aria-hidden
          className="torrents-cat-block"
          style={{ background: catColor(t.category_id) }}
          title={dict.torrents.categories[t.category_id] ?? ""}
        />
      </td>
      {/* 封面（好学站 46px 外链图；无图回退类型色块底 + 🎬） */}
      <td className="torrents-td-cover">
        <Link href={`/torrent/${id}`} aria-hidden tabIndex={-1}>
          {t.poster ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={t.poster}
              alt=""
              loading="lazy"
              className="torrents-cover"
            />
          ) : (
            <span
              className="torrents-cover torrents-cover--fallback"
              style={{ background: catColor(t.category_id) }}
            >
              🎬
            </span>
          )}
        </Link>
      </td>
      {/* 标题（好学站三行结构） */}
      <td className="torrents-td-title">
        <div className="torrents-title">
          {t.sticky && (
            <span className="torrents-pin" title={dict.torrent.sticky}>
              📌
            </span>
          )}
          <Link href={`/torrent/${id}`} className="torrents-name" title={t.name}>
            <b>{t.name}</b>
          </Link>
          {isNew && <span className="torrents-new">{dict.torrents.newTag ?? "新"}</span>}
          {/* 促销状态 + 剩余时间：紧跟种子名（好学站口径） */}
          {promo && (
            <span
              className={`torrents-promo${promo.key === "free" ? " torrents-promo--free" : ""}`}
              title={left ? `${dict.promotion[promo.key]} · ${left}` : dict.promotion[promo.key]}
            >
              {dict.promotion[promo.key]}
            </span>
          )}
          {left && <span className="torrents-left">剩余 {left}</span>}
        </div>
        {/* 副题链：学段 · 媒介 · 版本 · 小备注 */}
        {(grade || edition || subtitle) && (
          <div className="torrents-subtitle" title={subtitle}>
            {grade}
            {grade && edition ? " · " : ""}
            {edition}
            {subtitle ? `${grade || edition ? " · " : ""}${subtitle}` : ""}
          </div>
        )}
        {/* 第三行：标签在前、发布者在后（好学站标签色块 + 上传者口径） */}
        <div className="torrents-meta">
          <span className="torrents-tags">
            {t.official && <span className="torrents-tag torrents-tag--official">官方</span>}
            {t.sticky && <span className="torrents-tag torrents-tag--sticky">{dict.torrent.sticky}</span>}
          </span>
          {t.anonymous ? (
            <span className="text-sub">{dict.torrent.anonymous}</span>
          ) : (
            <span className="torrents-uploader">{t.owner_name ?? "—"}</span>
          )}
          <span className="torrents-meta-date">
            {new Date(t.created_at).toLocaleDateString(dateLocale(locale))}
          </span>
        </div>
      </td>
      <td className="num">{t.comments}</td>
      <td className="num torrents-td-alive" title={dict.torrents.alive}>
        {alive}
      </td>
      <td className="num">{formatBytes(t.size)}</td>
      <td className="num seed-arrow">{t.seeders}</td>
      <td className="num leech-arrow">{t.leechers}</td>
      <td className="num">{t.times_completed}</td>
      {extra}
      {/* 行为列：下载 + ⋮ 下拉（收藏/编辑/删除，好学站 staff 菜单口径） */}
      <td className="torrents-td-actions">
        <TorrentActions torrentId={id} downloadLabel={dict.torrents.download ?? "下载本种"} />
      </td>
    </tr>
  );
}

export { TorrentTr, catColor };
