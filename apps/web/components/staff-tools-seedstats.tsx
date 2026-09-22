"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

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
  const { dict } = useI18n();
  const t = dict.adminSeedStats;
  const [seedStats, setSeedStats] = useState<SeedStats | null>(null);

  useEffect(() => {
    api
      .get<SeedStats>("/api/v1/seed-stats")
      .then(setSeedStats)
      .catch(() => setSeedStats(null));
  }, []);

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">{t.title}</h2>
      <p className="mb-3 text-xs text-sub">{t.intro}</p>
      {seedStats ? (
        <>
          <div className="mb-4 grid grid-cols-1 gap-2 sm:grid-cols-3">
            {[
              [t.statSeeders, seedStats.seeders],
              [t.statTorrents, seedStats.seeding_torrents],
              [
                t.statAvgHours,
                fmt(t.hoursUnit, { n: seedStats.avg_seed_hours }),
              ],
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
              <h3 className="mb-2 text-sm font-bold">{t.topByCount}</h3>
              <table className="nexus-table w-full text-xs">
                <thead>
                  <tr>
                    <td className="colhead w-12">#</td>
                    <td className="colhead">{t.colUser}</td>
                    <td className="colhead w-24">{t.colSeeding}</td>
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
                        {t.noData}
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
            <div>
              <h3 className="mb-2 text-sm font-bold">{t.topByHours}</h3>
              <table className="nexus-table w-full text-xs">
                <thead>
                  <tr>
                    <td className="colhead w-12">#</td>
                    <td className="colhead">{t.colUser}</td>
                    <td className="colhead w-28">{t.colHours}</td>
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
                        {t.noData}
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </>
      ) : (
        <p className="py-4 text-center text-sub">{t.loadFail}</p>
      )}
    </section>
  );
}
