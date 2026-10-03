/**
 * 种子列表页表格（从 app/(main)/torrents/page.tsx 按域拆出）：
 * TorrentsTable 种子九列表格（参考站 colhead 图标表头，同列再点反转升降序）。
 * 无 hooks：保持 server component。
 * E6 视图布局：hiddenCols 站点级列隐藏（title/选择/行为列恒渲染）；
 * 只裁显隐不改顺序——列序是油猴脚本/爬虫的 DOM 契约。
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
  catIcons,
  hiddenCols,
}: {
  dict: Dict;
  sp: TorrentsSP;
  items: TorrentListItem[];
  catIcons?: Record<number, string>;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
  toggleSort: (cur: string | undefined, key: string) => string | undefined;
  /** E6：站点级隐藏列集合（空集=全显示） */
  hiddenCols?: Set<string>;
}) {
  const t = dict.torrents;
  const hd = hiddenCols ?? new Set<string>();
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
              {!hd.has("cat") && <th className="w-12">{t.colType}</th>}
              {!hd.has("cover") && (
                <th className="w-16" aria-label="封面" />
              )}
              <th>
                <a href={withParam(sp, "sort", toggleSort(sp.sort, "name"))}>
                  {t.colTitle}
                </a>
              </th>
              {/* 表头点击排序（NP colhead 口径）：同列再点反转升降序 */}
              {!hd.has("comments") && (
                <th className="w-16" title={t.colComments}>
                  <a
                    href={withParam(
                      sp,
                      "sort",
                      toggleSort(sp.sort, "comments"),
                    )}
                    className="inline-flex items-center gap-1"
                  >
                    <Icon name="messages" size={15} />
                    <span className="sr-only">{t.colComments}</span>
                  </a>
                </th>
              )}
              {!hd.has("alive") && (
                <th className="w-20" title={t.alive}>
                  <Icon
                    name="clock"
                    size={15}
                    className="inline align-[-3px]"
                  />
                  <span className="sr-only">{t.alive}</span>
                </th>
              )}
              {/* 2026-10-03：表头此前是裸 emoji（💾/🌱/⬇️/✅），列名只
                  藏在 title 属性里，扫读时无法判断哪列是什么。改为
                  图标 + 可见文字，窄屏由 CSS 隐藏文字保留图标。 */}
              {!hd.has("size") && (
                <th className="w-20" title={t.colSize}>
                  <a
                    className="inline-flex items-center gap-1"
                    href={withParam(sp, "sort", toggleSort(sp.sort, "size"))}
                  >
                    <Icon name="disc" size={15} />
                    <span className="tcol-label">{t.colSize}</span>
                  </a>
                </th>
              )}
              {!hd.has("seeders") && (
                <th className="w-16" title={t.colSeeders}>
                  <a
                    className="inline-flex items-center gap-1"
                    href={withParam(sp, "sort", toggleSort(sp.sort, "seeders"))}
                  >
                    <Icon name="seed" size={15} />
                    <span className="tcol-label">{t.colSeeders}</span>
                  </a>
                </th>
              )}
              {!hd.has("leechers") && (
                <th className="w-16" title={t.colLeechers}>
                  <a
                    className="inline-flex items-center gap-1"
                    href={withParam(
                      sp,
                      "sort",
                      toggleSort(sp.sort, "leechers"),
                    )}
                  >
                    <Icon name="download" size={15} />
                    <span className="tcol-label">{t.colLeechers}</span>
                  </a>
                </th>
              )}
              {!hd.has("completed") && (
                <th className="w-16" title={t.colCompleted}>
                  <a
                    className="inline-flex items-center gap-1"
                    href={withParam(
                      sp,
                      "sort",
                      toggleSort(sp.sort, "completed"),
                    )}
                  >
                    <Icon name="check" size={15} />
                    <span className="tcol-label">{t.colCompleted}</span>
                  </a>
                </th>
              )}
              <th className="w-24">{t.colActions}</th>
            </tr>
          </thead>
          <tbody>
            {items.map((t) => (
              <TorrentTr
                key={t.id}
                t={t}
                selectable
                catIcons={catIcons}
                hiddenCols={hiddenCols}
              />
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
