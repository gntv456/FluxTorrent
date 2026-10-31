/**
 * 作品内种子行（样图：作品卡下半截的 7 列表 —— 种子 / 时间 / 大小 /
 * 做种 / 下载 / 完成 / 操作）。
 *
 * 两处复用：资源页「作品聚合」的「宽松」形态（每部作品挂前 8 条）与作品
 * 详情页（该作品的全部种子）。行数据就是 `/api/v1/works` 下发的
 * TorrentRow，形状与 `/torrents` 列表行一致，故 chip 解析、体积/时间格式化、
 * 操作列全部沿用现有实现，不另造一套。
 *
 * 每行是客户端组件 WorkSeedRow：左侧箭头就地展开版本说明/发布者/截图。
 * 表头是本文件的（服务端），首列留空与箭头对齐。
 */

import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import { WorkSeedRow } from "@/components/work-seed-row";
import type { Dict } from "@/i18n/zh-CN";

/** 表头 + 若干行；`total > items.length` 时右侧给「其余 N 个版本」。 */
export function WorkSeedRows({
  items,
  dict,
  total,
  moreHref,
}: {
  items: TorrentListItem[];
  dict: Dict;
  /** 作品内种子总数（缺省 = items.length，不显示「其余」） */
  total?: number;
  /** 「其余」落点（作品详情页） */
  moreHref?: string;
}) {
  const t = dict.torrents;
  const n = total ?? items.length;
  const rest = Math.max(0, n - items.length);
  // 倒计时基准：服务端定死，客户端行只拿同一个 now 复算
  const now = Date.now();
  return (
    <>
      <div className="wk-shead">
        <span aria-hidden />
        <span className="wk-name">{t.colTitle}</span>
        <span className="wk-time">{dict.torrents2.dateLegend}</span>
        <span className="wk-size">{t.colSize}</span>
        <span className="wk-seed">{t.colSeeders}</span>
        <span className="wk-leech">{t.colLeechers}</span>
        <span className="wk-done">{t.colCompleted}</span>
        <span className="wk-act">{t.colActions}</span>
      </div>
      {items.map((row) => (
        <WorkSeedRow key={row.id} t={row} now={now} />
      ))}
      {rest > 0 && moreHref && (
        <div className="wk-more">
          <Link href={moreHref}>
            {dict.works.moreVersions.replace("{n}", String(rest))} ›
          </Link>
        </div>
      )}
    </>
  );
}
