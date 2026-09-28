/**
 * 种子列表表格/行卡片双视图块（E6 从 page.tsx 拆出，300 行门禁）：
 * md+ 表格、<md 行卡片，共享同一份 items 与列隐藏配置。
 */

import { TorrentsTable } from "./torrents-table";
import { TorrentRowList } from "@/components/torrent-row-card";
import type { Dict } from "@/i18n/zh-CN";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { TorrentsSP } from "./torrents-utils";

export function TorrentsTableView({
  dict,
  sp,
  items,
  withParam,
  toggleSort,
  catIcons,
  catColors,
  hiddenCols,
}: {
  dict: Dict;
  sp: TorrentsSP;
  items: TorrentListItem[];
  catIcons?: Record<number, string>;
  catColors?: Record<number, string>;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
  toggleSort: (cur: string | undefined, key: string) => string | undefined;
  /** E6：站点级列隐藏（空集=全显示） */
  hiddenCols?: Set<string>;
}) {
  return (
    <>
      <div className="hidden md:block">
        <TorrentsTable
          dict={dict}
          sp={sp}
          items={items}
          withParam={withParam}
          toggleSort={toggleSort}
          catIcons={catIcons}
          hiddenCols={hiddenCols}
        />
      </div>
      <div className="md:hidden">
        <TorrentRowList
          items={items}
          colors={catColors}
          hiddenCols={hiddenCols}
        />
      </div>
    </>
  );
}
