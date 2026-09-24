import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { PreserveItem } from "@/lib/data";
import { formatBytes, promotionBadge } from "@/lib/format";
import { getDict } from "@/i18n/server";
import {
  byId,
  catColor,
  dictName,
  getTorrentDicts,
  legacyDimName,
} from "@/lib/site-profile";
import { dateLocale } from "@/i18n/config";
import { TorrentActions } from "@/components/torrent-actions";
import { BatchCheckbox } from "@/components/torrent-batch";
import { Icon, ICON_NAMES } from "@/components/icons";

/** 已注册图标名（站长可手填 icon_key，未注册的名字不能当图标用） */
const KNOWN_ICONS = new Set<string>(ICON_NAMES as readonly string[]);

/** 保种区行（/preserve 下发的同构行：id 键为 torrent_id） */
type PreserveRowAlias = PreserveItem;

/** 促销行高亮（好学站口径：免费=浅蓝 #89c9e6 系、2x=暖黄系） */
function promoRowBg(promo: string | null | undefined): string | undefined {
  if (promo === "free" || promo === "x2free") return "var(--promo-free-bg)";
  if (promo === "x2" || promo === "x2half") return "var(--promo-x2-bg)";
  return undefined;
}

/** NP 口径行级语义类（DOM 契约，六维阶段三）：油猴脚本/爬虫按类定位行状态。
 *  pro_free/pro_2x/pro_50%/pro_free2up 与 NexusPHP torrents.php 同名同义；
 *  叠加时取「免」优先（与促销徽标 KIND_RANK 的高优先级一致）。 */
function promoSemanticClass(
  promo: string | null | undefined,
): string | undefined {
  switch (promo) {
    case "x2free":
      return "pro_free pro_2up";
    case "free":
      return "pro_free";
    case "x2half":
      return "pro_50pct pro_2up";
    case "x2":
      return "pro_2up";
    case "half":
      return "pro_50pct";
    case "p30":
      return "pro_30pct";
    default:
      return undefined;
  }
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
  selectable,
  catIcons,
}: {
  t: TorrentListItem | PreserveRowAlias;
  /** 行尾附加列（保种区的认领人/认领按钮）；渲染在数字列后、行为列前 */
  extra?: React.ReactNode;
  /** 列表页批量下载：首列渲染选择框（表格视图专用，保种区等复用方不传） */
  selectable?: boolean;
  /** 分类图标键（0166）：site-profile categories 下发；缺省回落首字色块 */
  catIcons?: Record<number, string>;
}) {
  const icons = catIcons ?? {};
  const { dict, locale } = await getDict();
  const dicts = await getTorrentDicts();
  const catNames = byId(dicts.categories);
  const id = "torrent_id" in t ? t.torrent_id : t.id;
  const promo = promotionBadge(
    (t.promotion as TorrentListItem["promotion"]) ?? null,
  );
  const edition = legacyDimName(dicts.editions, t.edition_id);
  const grade = legacyDimName(dicts.grades, t.grade_id);
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
  // DOM 契约（NP 口径）：pro_* 促销态 + 断种/官种行级标记
  const semanticCls = [
    promoSemanticClass(t.promotion),
    t.seeders === 0 ? "torrent-dead" : undefined,
    t.official ? "torrent-official" : undefined,
    t.sticky ? "torrent-sticky" : undefined,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <tr
      data-torrent-id={id}
      className={semanticCls || undefined}
      style={rowBg ? { background: rowBg } : undefined}
    >
      {selectable && (
        <td className="torrents-td-ck">
          <BatchCheckbox id={id} />
        </td>
      )}
      {/* 类型（0166：分类图标默认套，站长后台 icon_key 可替换；空键回落首字色块） */}
      <td className="torrents-td-cat">
        <span
          aria-hidden
          className="torrents-cat-block"
          style={{ background: catColor(dicts.colors, t.category_id) }}
          title={catNames[t.category_id] ?? ""}
        >
          {(() => {
            // 图标键是站长手填的：未注册的名字 Icon 会渲染 null，
            // 那样色块就空了——必须自己判存在，缺失时回落分类名首字
            const ico = icons[t.category_id];
            return ico && KNOWN_ICONS.has(ico) ? (
              <Icon name={ico} size={22} className="torrents-cat-ico" />
            ) : (
              (catNames[t.category_id] ?? "?").slice(0, 1)
            );
          })()}
        </span>
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
              style={{ background: catColor(dicts.colors, t.category_id) }}
            >
              <Icon name="disc" size={20} />
            </span>
          )}
        </Link>
      </td>
      {/* 标题（好学站三行结构） */}
      <td className="torrents-td-title">
        <div className="torrents-title">
          {t.sticky && (
            <span className="torrents-pin" title={dict.torrent.sticky}>
              <Icon name="pin" size={13} />
            </span>
          )}
          <Link
            href={`/torrent/${id}`}
            className="torrents-name"
            title={t.name}
          >
            <b>{t.name}</b>
          </Link>
          {isNew && (
            <span className="torrents-new">{dict.torrents.newTag}</span>
          )}
          {/* 0170 审核状态徽标：仅待审(0)/被拒(2)行显示（站点开关放行后进列表；
              保种区等复用方行类型无此字段，in 收窄容错） */}
          {"approval_status" in t &&
            (t.approval_status === 0 || t.approval_status === 2) && (
              <span
                className={`torrents-approval${t.approval_status === 2 ? " torrents-approval--rejected" : ""}`}
              >
                {t.approval_status === 0
                  ? dict.torrents.statusPending
                  : dict.torrents.statusRejected}
              </span>
            )}
          {/* 促销状态 + 剩余时间：紧跟种子名（好学站口径） */}
          {promo && (
            <span
              className={`torrents-promo${promo.key === "free" ? " torrents-promo--free" : ""}`}
              title={
                left
                  ? `${dict.promotion[promo.key]} · ${left}`
                  : dict.promotion[promo.key]
              }
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
            {t.official && (
              <span className="torrents-tag torrents-tag--official">
                {dict.torrents.officialTag}
              </span>
            )}
            {t.sticky && (
              <span className="torrents-tag torrents-tag--sticky">
                {dict.torrent.sticky}
              </span>
            )}
            {/* 用户标签徽标（0159 P1）：随行回显 ≤3 个 + 溢出计数，
                消费字典样式列（与论坛 TagChip 同口径） */}
            {(t.tags ?? []).slice(0, 3).map((b) => (
              <span
                key={b.id}
                className="torrents-tag torrents-tag--user"
                style={
                  b.bg_color
                    ? {
                        background: b.bg_color,
                        color: b.color || undefined,
                      }
                    : undefined
                }
              >
                {b.name}
              </span>
            ))}
            {(t.tags?.length ?? 0) > 3 && (
              <span
                className="torrents-tag torrents-tag--user torrents-tag--more"
                title={(t.tags ?? [])
                  .slice(3)
                  .map((b) => b.name)
                  .join(" / ")}
              >
                +{t.tags!.length - 3}
              </span>
            )}
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
        <TorrentActions torrentId={id} downloadLabel={dict.torrents.download} />
      </td>
    </tr>
  );
}

export { TorrentTr, promoSemanticClass };
