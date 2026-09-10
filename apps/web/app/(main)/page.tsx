import Link from "next/link";
import { api, paged } from "@/lib/api-client";
import { TorrentTr } from "@/components/torrent-table";
import { getDict } from "@/i18n/server";
import type { Page, TorrentListItem } from "@fluxtorrent/domain-types";
import { HomeSections } from "@/components/home-sections";

export const dynamic = "force-dynamic";

/** 首页（包子站 index.php 像素级复刻）：
 *  社区新鲜事 + 签到日历 → 新增资源统计图表 → 站点数据三列 + 幸运大转盘 → 免责/友链 → 最新种子 */
export default async function HomePage() {
  const { dict } = await getDict();
  let latest: Page<TorrentListItem> | null = null;
  try {
    latest = await paged<TorrentListItem>("/api/v1/torrents", { limit: 10 });
  } catch {
    // 后端未启动时首页降级为空态（本地开发体验）
  }

  return (
    <div className="flex flex-col gap-4">
      <HomeSections />

      {/* 最新种子（table.torrents 复刻：colhead 标题行 + 列头行） */}
      {latest && latest.items.length > 0 && (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead" colSpan={8}>
                <div className="flex items-baseline justify-between">
                  <h2 className="font-display">{dict.home.latest}</h2>
                  <Link href="/torrents" className="text-xs">
                    {dict.home.viewAll}
                  </Link>
                </div>
              </td>
            </tr>
            <tr>
              <th className="w-12">{dict.torrents.colType}</th>
              <th>{dict.torrents.colTitle}</th>
              <th className="w-16">{dict.torrents.colComments}</th>
              <th className="w-20">{dict.torrents.colSize}</th>
              <th className="w-16">{dict.torrents.colSeeders}</th>
              <th className="w-16">{dict.torrents.colLeechers}</th>
              <th className="w-16">{dict.torrents.colCompleted}</th>
              <th className="w-28">{dict.torrents.colOwner}</th>
            </tr>
          </thead>
          <tbody>
            {latest.items.map((t) => (
              <TorrentTr key={t.id} t={t} />
            ))}
          </tbody>
        </table>
      )}
      {latest && latest.items.length === 0 && (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.home.latest}</td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="p-6 text-center text-sub">{dict.home.empty}</td>
            </tr>
          </tbody>
        </table>
      )}
    </div>
  );
}
