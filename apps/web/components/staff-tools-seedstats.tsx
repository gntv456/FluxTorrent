"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";

/** 保种统计面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  seed.stats.view——站点做种总览与 Top 保种用户。 */

interface SeedStats {
  seeders: number;
  seeding_torrents: number;
  avg_seed_hours: number;
  top_by_count: { user_id: number; username: string; seeding: number }[];
  top_by_hours: { user_id: number; username: string; hours: number }[];
}

export function StaffSeedStatsPanel() {
  const [seedStats, setSeedStats] = useState<SeedStats | null>(null);

  useEffect(() => {
    api
      .get<SeedStats>("/api/v1/seed-stats")
      .then(setSeedStats)
      .catch(() => setSeedStats(null));
  }, []);

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">保种统计</h2>
      <p className="mb-3 text-xs text-sub">
        站点做种总览与 Top 保种用户。持有「保种统计」权限者可见（保种员 / 贵宾 /
        管理组）。
      </p>
      {seedStats ? (
        <>
          <div className="mb-4 grid grid-cols-1 gap-2 sm:grid-cols-3">
            {[
              ["做种用户", seedStats.seeders],
              ["做种条目", seedStats.seeding_torrents],
              ["平均做种时长", `${seedStats.avg_seed_hours} 小时`],
            ].map(([label, v]) => (
              <div
                key={String(label)}
                className="rounded-[var(--r-md)] border border-line bg-[var(--surface-raised)] p-3 text-center"
              >
                <p className="text-xs text-sub">{label}</p>
                <p className="num mt-1 text-2xl text-ink">
                  {typeof v === "number" ? v.toLocaleString() : v}
                </p>
              </div>
            ))}
          </div>
          <div className="baozi-wide-table-scroll grid grid-cols-1 gap-4 md:grid-cols-2">
            <div>
              <h3 className="mb-2 text-sm font-bold">做种数 Top 10</h3>
              <table className="nexus-table w-full text-xs">
                <thead>
                  <tr>
                    <td className="colhead w-12">#</td>
                    <td className="colhead">用户</td>
                    <td className="colhead w-24">做种数</td>
                  </tr>
                </thead>
                <tbody>
                  {seedStats.top_by_count.map((r, i) => (
                    <tr key={r.user_id}>
                      <td className="rowfollow num">{i + 1}</td>
                      <td className="rowfollow">
                        <a
                          href={`/user/${r.user_id}`}
                          className="hover:text-sky"
                        >
                          {r.username}
                        </a>
                      </td>
                      <td className="rowfollow num">{r.seeding}</td>
                    </tr>
                  ))}
                  {seedStats.top_by_count.length === 0 && (
                    <tr>
                      <td colSpan={3} className="py-3 text-center text-sub">
                        暂无数据
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
            <div>
              <h3 className="mb-2 text-sm font-bold">做种时长 Top 10</h3>
              <table className="nexus-table w-full text-xs">
                <thead>
                  <tr>
                    <td className="colhead w-12">#</td>
                    <td className="colhead">用户</td>
                    <td className="colhead w-28">累计小时</td>
                  </tr>
                </thead>
                <tbody>
                  {seedStats.top_by_hours.map((r, i) => (
                    <tr key={r.user_id}>
                      <td className="rowfollow num">{i + 1}</td>
                      <td className="rowfollow">
                        <a
                          href={`/user/${r.user_id}`}
                          className="hover:text-sky"
                        >
                          {r.username}
                        </a>
                      </td>
                      <td className="rowfollow num">{r.hours}</td>
                    </tr>
                  ))}
                  {seedStats.top_by_hours.length === 0 && (
                    <tr>
                      <td colSpan={3} className="py-3 text-center text-sub">
                        暂无数据
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </>
      ) : (
        <p className="py-4 text-center text-sub">
          加载失败或无「保种统计」权限
        </p>
      )}
    </section>
  );
}
