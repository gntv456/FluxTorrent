import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type { PreserveItem } from "@/lib/data";
import { formatBytes } from "@/lib/format";

/** 数字六列（E6 视图布局：hd 命中即不渲染；title 之外的显隐全走这里）。
 *  从 torrent-table.tsx 拆出（300 行门禁）。 */
export function TorrentNums({
  t,
  hd,
  aliveTitle,
  alive,
}: {
  t: TorrentListItem | PreserveItem;
  hd: Set<string>;
  aliveTitle: string;
  alive: string;
}) {
  return (
    <>
      {!hd.has("comments") && (
        <td className="num">{(t as TorrentListItem).comments}</td>
      )}
      {!hd.has("alive") && (
        <td className="num torrents-td-alive" title={aliveTitle}>
          {alive}
        </td>
      )}
      {!hd.has("size") && (
        <td className="num">{formatBytes(t.size)}</td>
      )}
      {!hd.has("seeders") && (
        <td className="num seed-arrow">{t.seeders}</td>
      )}
      {!hd.has("leechers") && (
        <td className="num leech-arrow">{t.leechers}</td>
      )}
      {!hd.has("completed") && (
        <td className="num">{t.times_completed}</td>
      )}
    </>
  );
}
