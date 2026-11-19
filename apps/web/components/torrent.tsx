import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import { formatBytes, promotionBadge } from "@/lib/format";
import { getDict } from "@/i18n/server";
import {
  catColor,
  getTorrentDicts,
} from "@/lib/site-profile";
import { Icon } from "@/components/icons";
import { dateLocale, fmt } from "@/i18n/config";
import { Badge } from "@/components/ui/badge";
import { EmptyStateMetric } from "@/components/ux-metric-beacon";

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
      className={
        "flex items-center gap-3 rounded-[var(--r-md)] border " +
        "border-line bg-[var(--surface-card)] p-3 " +
        "shadow-[var(--shadow-card)] transition-transform " +
        "hover:-translate-y-0.5 active:scale-[0.99]"
      }
    >
      {/* 左：分类色块（34px 圆角 9px） */}
      <span
        aria-hidden
        className="h-[var(--tap-min)] w-[var(--tap-min)] shrink-0 rounded-[9px]"
        style={{ background: catColor(dicts.colors, t.category_id) }}
      />
      {/* 中：主区 */}
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5">
          {t.sticky && (
            <Badge tone="warn">
              {dict.torrent.sticky}
            </Badge>
          )}
          <span className="truncate font-bold text-ink">{t.name}</span>
          {promo && (
            <Badge tone={promo.tone} variant={promo.variant}>
              {dict.promotion[promo.key]}
            </Badge>
          )}
          {t.official && (
            <Badge tone="brand" variant="solid">
              {dict.torrent.official}
            </Badge>
          )}
        </div>
        {t.small_descr && (
          <p className="mt-0.5 truncate text-xs text-sub">{t.small_descr}</p>
        )}
        {/* 维度链（R3-三步）：sections 单源——站型任意维度名拼接 */}
        {(t.sec_names?.length ?? 0) > 0 && (
          <p className="mt-0.5 flex flex-wrap gap-1.5 text-xs text-sub">
            {(t.sec_names ?? []).slice(0, 3).map((n, i) => (
              <span key={i}>{n}</span>
            ))}
          </p>
        )}
        <p className="mt-0.5 text-xs text-sub">
          {t.anonymous ? dict.torrent.anonymous : (t.owner_name ?? "—")} ·{" "}
          {new Date(t.created_at).toLocaleDateString(dateLocale(locale))}
        </p>
      </div>
      {/* 右：数据列（tabular-nums；做种绿/下载橙/完成灰） */}
      <div className="num flex shrink-0 flex-col items-end gap-0.5 text-xs">
        <span className="text-sub">{formatBytes(t.size)}</span>
        <span className="text-up">
          {dict.torrent.seeding} {t.seeders}
        </span>
        <span className="text-down">
          {dict.torrent.leeching} {t.leechers}
        </span>
        <span className="text-sub">
          {dict.torrent.completed} {t.times_completed}
        </span>
      </div>
    </Link>
  );
}

/** 空态四态（A4）：allEmpty / filter / search / dead。
 *  context 缺省时按旧 props 推断，兼容既有调用：search → search、
 *  aliveFiltered（仅活种视图）→ dead、其余 → allEmpty。
 *  四态主 CTA 统一走 .ux-empty__cta（brand 令牌），不使用危险色。 */
export type EmptyTorrentsContext = "allEmpty" | "filter" | "search" | "dead";

export async function EmptyTorrents({
  aliveFiltered = false,
  search,
  clearHref = "/torrents",
  context,
}: {
  aliveFiltered?: boolean;
  /** P1-8：搜索无结果要独立文案，不能复用「站里没种子」的空态。 */
  search?: string;
  clearHref?: string;
  /** A4：页面按 URL 推断后显式传入；不传则按旧口径推断。 */
  context?: EmptyTorrentsContext;
}) {
  const { dict } = await getDict();
  const ctx: EmptyTorrentsContext =
    context ?? (search ? "search" : aliveFiltered ? "dead" : "allEmpty");

  const title =
    ctx === "filter"
      ? dict.uxOpt.emptyFilterTitle
      : ctx === "search"
        ? fmt(dict.torrent.emptySearchTitle, { q: search ?? "" })
        : ctx === "dead"
          ? dict.torrent.emptyAliveTitle
          : dict.torrent.emptyTitle;
  // 只有 filter / search 有对应 i18n 描述键；其余两态不写死文案（四语纪律）
  const desc =
    ctx === "filter"
      ? dict.uxOpt.emptyFilterDesc
      : ctx === "search"
        ? dict.torrent.emptySearchHint
        : null;
  const cta =
    ctx === "allEmpty"
      ? { text: dict.common.publish, href: "/upload" }
      : ctx === "dead"
        ? { text: dict.torrent.emptyAliveAction, href: "/torrents?alive=0" }
        : { text: dict.uxOpt.clearFilters, href: clearHref };

  return (
    <div
      className="torrents-empty ux-empty flex flex-col items-center gap-3
        py-16 text-center"
    >
      <EmptyStateMetric context={ctx} />
      <Icon
        name={ctx === "search" ? "search" : "seed"}
        size={ctx === "search" ? 64 : 72}
        className={
          ctx === "search"
            ? "text-[var(--text-faint)]"
            : "text-[var(--sky)]"
        }
      />
      <h2 className="font-display text-xl text-ink">{title}</h2>
      {desc && <p className="ux-empty__desc">{desc}</p>}
      <a href={cta.href} className="ux-empty__cta">
        {cta.text}
      </a>
      {/* 0503：搜索无结果的第二出口——站里真没有时求种是天然下一步 */}
      {ctx === "search" && search && (
        <a
          href={`/requests?search=${encodeURIComponent(search)}`}
          className="text-link text-xs"
        >
          {dict.torrent.emptyGoRequest} →
        </a>
      )}
    </div>
  );
}
