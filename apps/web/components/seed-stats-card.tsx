"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface SeedStats {
  seeders: number;
  seeding_torrents: number;
  avg_seed_hours: number;
  top_by_count: { user_id: number; username: string; seeding: number }[];
  top_by_hours: { user_id: number; username: string; hours: number }[];
}

/**
 * 保种统计卡片（前台 /preserve 用）。
 *
 * 端点 /seed-stats 刻意**不设 staff 门槛**——保种员 / 贵宾这类非管理组角色
 * 也能访问；管理面板里的「保种统计」页签则是管理组视角的同一份数据。
 * 无权限时静默降级为提示文案，不打扰普通用户。
 */
export function SeedStatsCard() {
  const { dict } = useI18n();
  const [data, setData] = useState<SeedStats | null>(null);
  const [state, setState] = useState<"loading" | "ok" | "denied">("loading");

  useEffect(() => {
    let alive = true;
    api
      .get<SeedStats>("/api/v1/seed-stats")
      .then((d) => {
        if (!alive) return;
        setData(d);
        setState("ok");
      })
      .catch(() => {
        if (alive) setState("denied");
      });
    return () => {
      alive = false;
    };
  }, []);

  if (state === "loading") {
    return (
      <section className="baozi-panel p-4">
        <p className="text-sm text-sub">加载中…</p>
      </section>
    );
  }

  if (state === "denied" || !data) {
    return (
      <section className="baozi-panel p-4">
        <h2 className="mb-1 text-base font-bold">保种统计</h2>
        <p className="text-xs text-sub">
          需「保种统计」权限（保种员 / 贵宾 / 管理组）方可查看站点做种总览。
        </p>
      </section>
    );
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-3 text-base font-bold">保种统计</h2>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        {[
          ["做种用户", data.seeders.toLocaleString()],
          ["做种条目", data.seeding_torrents.toLocaleString()],
          ["平均做种时长", `${data.avg_seed_hours} 小时`],
        ].map(([label, v]) => (
          <div
            key={label}
            className="rounded-[var(--r-md)] border border-line bg-[var(--surface-raised)] p-3 text-center"
          >
            <p className="text-xs text-sub">{label}</p>
            <p className="num mt-1 text-xl text-ink">{v}</p>
          </div>
        ))}
      </div>
      {data.top_by_count.length > 0 && (
        <div className="mt-4">
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
              {data.top_by_count.map((r, i) => (
                <tr key={r.user_id}>
                  <td className="rowfollow num">{i + 1}</td>
                  <td className="rowfollow">
                    <a href={`/user/${r.user_id}`} className="hover:text-sky">
                      {r.username}
                    </a>
                  </td>
                  <td className="rowfollow num">{r.seeding}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
