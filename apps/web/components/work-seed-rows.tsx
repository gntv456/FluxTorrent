/**
 * 作品内种子行（样图：作品卡下半截的 7 列表 —— 种子 / 时间 / 大小 /
 * 做种 / 下载 / 完成 / 操作）。
 *
 * 两处复用：资源页「作品聚合」视图（每部作品挂前 8 条）与作品详情页
 * （该作品的全部种子）。行数据就是 `/api/v1/works` 下发的 TorrentRow，
 * 形状与 `/torrents` 列表行一致，故 chip 解析、体积/时间格式化、操作列
 * 全部沿用现有实现，不另造一套。
 *
 * 服务端组件（无 hooks）：操作列用既有的客户端组件 TorrentActions。
 */

import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import { formatBytes, promotionBadge } from "@/lib/format";
import { dateLocale } from "@/i18n/config";
import { TorrentActions } from "@/components/torrent-actions";
import { nameChips } from "@/components/work-chips";
import type { Dict } from "@/i18n/zh-CN";
import type { Locale } from "@/i18n/config";

/** 促销 → chip 样式（免费绿系、2x 暖系、其余蓝系） */
function promoCls(promotion: string | null): string {
  if (promotion === "free" || promotion === "x2free") return "wk-cfree";
  if (promotion === "x2" || promotion === "x2half") return "wk-cep";
  return "wk-cweb";
}

function Row({
  t,
  dict,
  locale,
}: {
  t: TorrentListItem;
  dict: Dict;
  locale: Locale;
}) {
  const promo = promotionBadge(t.promotion);
  const chips = nameChips(t.name, dict.works.chipSubs);
  const date = new Date(t.created_at).toLocaleDateString(dateLocale(locale));
  return (
    <div className="wk-row" data-torrent-id={t.id}>
      <div className="wk-name">
        <div className="wk-chips">
          {chips.map((c) => (
            <span key={c.text} className={`wk-chip ${c.cls}`}>
              {c.text}
            </span>
          ))}
          {promo && (
            <span className={`wk-chip ${promoCls(t.promotion)}`}>
              {dict.promotion[promo.key]}
            </span>
          )}
          {t.official && (
            <span className="wk-chip wk-cweb">
              {dict.torrents.officialTag}
            </span>
          )}
        </div>
        <div className="wk-rtitle">
          <Link href={`/torrent/${t.id}`} title={t.name}>
            {t.name}
          </Link>
        </div>
      </div>
      <span className="wk-time">{date}</span>
      <span className="wk-size">{formatBytes(t.size)}</span>
      <span className={`wk-seed${t.seeders === 0 ? " is-zero" : ""}`}>
        {t.seeders}
      </span>
      <span className="wk-leech">{t.leechers}</span>
      <span className="wk-done">{t.times_completed}</span>
      <div className="wk-act">
        <TorrentActions torrentId={t.id} torrentName={t.name} />
      </div>
    </div>
  );
}

/** 表头 + 若干行；`total > items.length` 时右侧给「查看全部」链接。 */
export function WorkSeedRows({
  items,
  dict,
  locale,
  total,
  moreHref,
}: {
  items: TorrentListItem[];
  dict: Dict;
  locale: Locale;
  /** 作品内种子总数（缺省 = items.length，不显示「查看全部」） */
  total?: number;
  /** 「查看全部」落点（作品详情页） */
  moreHref?: string;
}) {
  const t = dict.torrents;
  const n = total ?? items.length;
  return (
    <>
      <div className="wk-shead">
        <span className="wk-name">{t.colTitle}</span>
        <span className="wk-time">{t.alive}</span>
        <span className="wk-size">{t.colSize}</span>
        <span className="wk-seed">{t.colSeeders}</span>
        <span className="wk-leech">{t.colLeechers}</span>
        <span className="wk-done">{t.colCompleted}</span>
        <span className="wk-act">{t.colActions}</span>
      </div>
      {items.map((row) => (
        <Row key={row.id} t={row} dict={dict} locale={locale} />
      ))}
      {n > items.length && moreHref && (
        <div className="wk-more">
          <Link href={moreHref}>{dict.works.seedsInWork.replace(
              "{n}",
              String(n),
            )} →</Link>
        </div>
      )}
    </>
  );
}
