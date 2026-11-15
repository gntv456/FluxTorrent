"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
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
 *
 * 2026-11-15 紧凑化：三个大数字并成一行细条（不再撑一整块面板），
 * 榜单收进 details（默认收起——统计是辅线，不该抢保种资源表的注意力）。
 */
export function SeedStatsCard() {
  const { dict } = useI18n();
  const [data, setData] = useState<SeedStats | null>(null);
  const [state, setState] = useState<
    "loading" | "ok" | "denied" | "failed"
  >("loading");

  useEffect(() => {
    let alive = true;
    api
      .get<SeedStats>("/api/v1/seed-stats")
      .then((d) => {
        if (!alive) return;
        setData(d);
        setState("ok");
      })
      .catch((e) => {
        if (!alive) return;
        // 只有 403 才是「没权限」；网络/500 不能演成权限说明
        setState(
          e instanceof ApiError && (e.status === 403 || e.code === 2003)
            ? "denied"
            : "failed",
        );
      });
    return () => {
      alive = false;
    };
  }, []);

  if (state === "loading") return null;
  if (state === "failed") {
    return <p className="text-xs text-warn">{dict.common.loadFailed}</p>;
  }
  if (state === "denied" || !data) {
    return (
      <p className="text-xs text-sub">
        需「保种统计」权限（保种员 / 贵宾 / 管理组）方可查看站点做种总览。
      </p>
    );
  }

  const cells: [string, string][] = [
    ["做种用户", data.seeders.toLocaleString()],
    ["做种条目", data.seeding_torrents.toLocaleString()],
    ["平均做种时长", `${data.avg_seed_hours}h`],
  ];
  return (
    <details className="brand-panel">
      {/* 细条 summary：数字行内联，榜单折叠（紧凑化——统计是辅线） */}
      <summary
        className={
          "flex cursor-pointer select-none items-center gap-3 px-4 py-2 " +
          "text-sm font-bold text-[var(--text-brand)]"
        }
      >
        保种统计
        <span className="flex flex-wrap gap-x-4 gap-y-1 font-normal">
          {cells.map(([label, v]) => (
            <span key={label}>
              <b className="num text-[15px] text-[var(--text-strong)]">
                {v}
              </b>
              <small className="ml-1 text-[11.5px] text-faint">
                {label}
              </small>
            </span>
          ))}
        </span>
      </summary>
      {data.top_by_count.length > 0 && (
        <div className="px-4 pb-3">
          <h3 className="mb-2 text-sm font-bold">做种数 Top 10</h3>
          <div className="brand-wide-table-scroll">
            <table className="nexus-table w-full text-xs">
              <thead>
                <tr>
                  <th scope="col" className="colhead w-12">#</th>
                  <th scope="col" className="colhead">用户</th>
                  <th scope="col" className="colhead w-24">做种数</th>
                </tr>
              </thead>
              <tbody>
                {data.top_by_count.map((r, i) => (
                  <tr key={r.user_id}>
                    <td className="rowfollow num">{i + 1}</td>
                    <td className="rowfollow">
                      <a
                        href={`/user/${r.user_id}`}
                        className="hover:text-[var(--sky)]"
                      >
                        {r.username}
                      </a>
                    </td>
                    <td className="rowfollow num">{r.seeding}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </details>
  );
}
