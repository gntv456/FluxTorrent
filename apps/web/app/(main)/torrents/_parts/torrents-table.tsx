/**
 * 种子列表页表格（从 app/(main)/torrents/page.tsx 按域拆出）：
 * TorrentsTable 种子九列表格（参考站 colhead 图标表头，同列再点反转升降序）。
 * 无 hooks：保持 server component。
 */

import { TorrentTr } from "@/components/torrent-table";
import { BatchCheckAll, TorrentBatchBar } from "@/components/torrent-batch";
import { Icon } from "@/components/icons";
import type { Dict } from "@/i18n/zh-CN";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { TorrentsSP } from "./torrents-utils";

export function TorrentsTable({
  dict,
  sp,
  items,
  withParam,
  toggleSort,
}: {
  dict: Dict;
  sp: TorrentsSP;
  items: TorrentListItem[];
  withParam: (
    sp: TorrentsSP,
    key: string,
    value: string | undefined,
  ) => string;
  toggleSort: (cur: string | undefined, key: string) => string | undefined;
}) {
  const t = dict.torrents;
  return (
    <>
      {/* 批量下载（阶段三）：选中行后出现操作条；仅表格视图提供多选 */}
      <TorrentBatchBar />
      <div
        className="baozi-wide-table-scroll"
        role="region"
        aria-label={t.title}
      >
      <table className="nexus-table torrents-table">
        <thead>
          <tr>
            <th className="w-8">
              <BatchCheckAll />
            </th>
            <th className="w-12">{t.colType}</th>
            <th className="w-16" aria-label="封面" />
            <th>
              <a href={withParam(sp, "sort", toggleSort(sp.sort, "name"))}>
                {t.colTitle}
              </a>
            </th>
            {/* 表头点击排序（NP colhead 口径）：同列再点反转升降序 */}
            <th className="w-16" title={t.colComments}>
              <a
                href={withParam(sp, "sort", toggleSort(sp.sort, "comments"))}
                className="inline-flex"
              >
                <Icon name="messages" size={15} />
              </a>
            </th>
            <th className="w-20" title={t.alive}>
              <Icon name="clock" size={15} className="inline align-[-3px]" />
            </th>
            <th className="w-20" title={t.colSize}>
              <a href={withParam(sp, "sort", toggleSort(sp.sort, "size"))}>
                💾
              </a>
            </th>
            <th className="w-16" title={t.colSeeders}>
              <a href={withParam(sp, "sort", toggleSort(sp.sort, "seeders"))}>
                🌱
              </a>
            </th>
            <th className="w-16" title={t.colLeechers}>
              <a href={withParam(sp, "sort", toggleSort(sp.sort, "leechers"))}>
                ⬇️
              </a>
            </th>
            <th className="w-16" title={t.colCompleted}>
              <a
            href={withParam(sp, "sort", toggleSort(sp.sort, "completed"))}
          >
                ✅
              </a>
            </th>
            <th className="w-24">{t.colActions}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((t) => (
            <TorrentTr key={t.id} t={t} selectable />
          ))}
        </tbody>
      </table>
      </div>
    </>
  );
}
