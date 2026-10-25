/**
 * 种子列表的三种「种子粒度」视图（卡片 / 海报墙 / 表格）。
 *
 * 从 page.tsx 按域拆出（300 行门禁 + 作品聚合视图要往同一个分支里接第四种，
 * 留在页面里会把它顶出基线）。纯呈现：数据仍是列表接口那一份。
 */

import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";
import { TorrentCards, TorrentPosters } from "./torrents-cards";
import { TorrentsTableView } from "./torrents-table-view";
import type { TorrentsSP, TorrentView } from "./torrents-utils";

export function TorrentListViews({
  view,
  items,
  dict,
  sp,
  withParam,
  toggleSort,
  catIcons,
  colors,
  hiddenCols,
}: {
  /** 种子粒度视图（作品聚合不在这里，它走 TorrentWorks） */
  view: Exclude<TorrentView, "group">;
  items: TorrentListItem[];
  dict: Dict;
  sp: TorrentsSP;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
  toggleSort: (cur: string | undefined, key: string) => string | undefined;
  catIcons: Record<number, string>;
  colors: Record<number, string>;
  hiddenCols: Set<string>;
}) {
  if (view === "card") {
    return <TorrentCards items={items} dict={dict} colors={colors} />;
  }
  if (view === "poster") {
    return <TorrentPosters items={items} dict={dict} colors={colors} />;
  }
  // M3：<640 表格隐藏、行卡片接管（useIsCompact 客户端分支，
  // URL view 显式指定时上面已分流不会走到这里）。SSR 首帧
  // 渲染表格（fallback=桌面），挂载后双态切换。
  return (
    <TorrentsTableView
      dict={dict}
      sp={sp}
      items={items}
      withParam={withParam}
      toggleSort={toggleSort}
      catIcons={catIcons}
      catColors={colors}
      hiddenCols={hiddenCols}
    />
  );
}
