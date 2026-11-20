"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 运营聚合双看板（0506 运营体检二批，从 admin-ops-broadcast.tsx
 *  按域拆出守 300 门禁）：签到 14 天趋势 + H&R 状态分布/新增违规
 *  趋势。数据此前只有分页明细行，无任何聚合面。 */

interface AttendanceDay {
  date: string;
  checkins: number;
  makeups: number;
  avgStreak: number;
  activeUsers: number;
}

interface HrStats {
  distribution: { status: string; count: number }[];
  total: number;
  pardonRate: number;
  trend: { date: string; violations: number }[];
}

interface DonateMonth {
  month: string;
  orders: number;
  usd: number;
  avgUsd: number;
}

export function OpsBoards() {
  const { dict } = useI18n();
  const [attDays, setAttDays] = useState<AttendanceDay[] | null>(null);
  const [hr, setHr] = useState<HrStats | null>(null);
  const [donate, setDonate] = useState<DonateMonth[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [a, h, dn] = await Promise.all([
        api.get<{ days: AttendanceDay[] }>("/api/v1/admin/attendance-stats"),
        api.get<HrStats>("/api/v1/admin/hr-stats"),
        api.get<{ months: DonateMonth[] }>("/api/v1/admin/donate-stats"),
      ]);
      setAttDays(a.days ?? []);
      setHr(h);
      setDonate(dn.months ?? []);
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [dict]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-4">
      {err && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {err}
        </p>
      )}
      <div className="brand-panel p-4">
        <h2 className="mb-3 font-display text-lg">签到 14 天趋势</h2>
        <div className="brand-wide-table-scroll">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-line text-left text-xs text-sub">
                <th className="py-2">日期</th>
                <th className="py-2">签到</th>
                <th className="py-2">占比</th>
                <th className="py-2">平均连签</th>
                <th className="py-2">补签</th>
              </tr>
            </thead>
            <tbody>
              {(attDays ?? []).length === 0 && (
                <tr>
                  <td colSpan={5} className="py-6 text-center text-sub">
                    暂无数据
                  </td>
                </tr>
              )}
              {(attDays ?? []).map((d) => (
                <tr key={d.date} className="border-b border-line">
                  <td className="py-2">{d.date.slice(5)}</td>
                  <td className="py-2">{d.checkins}</td>
                  <td className="py-2 text-sub">
                    {d.activeUsers > 0
                      ? `${Math.round((d.checkins / d.activeUsers) * 100)}%`
                      : "—"}
                  </td>
                  <td className="py-2">{d.avgStreak}</td>
                  <td className="py-2 text-sub">{d.makeups}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      {hr && (
        <div className="brand-panel p-4">
          <h2 className="mb-3 font-display text-lg">
            H&R 状态分布（豁免率 {hr.pardonRate}%）
          </h2>
          <div className="mb-3 flex flex-wrap gap-2">
            {hr.distribution.map((d) => (
              <span
                key={d.status}
                className="rounded border border-line px-2 py-1 text-sm"
              >
                {d.status}: <b>{d.count}</b>
              </span>
            ))}
          </div>
          <div className="brand-wide-table-scroll">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-line text-left text-xs text-sub">
                  <th className="py-2">日期</th>
                  <th className="py-2">新增违规</th>
                </tr>
              </thead>
              <tbody>
                {hr.trend.map((t) => (
                  <tr key={t.date} className="border-b border-line">
                    <td className="py-2">{t.date.slice(5)}</td>
                    <td className="py-2">{t.violations}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
      {donate && donate.length > 0 && (
        <div className="brand-panel p-4">
          <h2 className="mb-3 font-display text-lg">捐赠 12 月趋势</h2>
          <div className="brand-wide-table-scroll">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-line text-left text-xs text-sub">
                  <th className="py-2">月份</th>
                  <th className="py-2">订单</th>
                  <th className="py-2">实收 USD</th>
                  <th className="py-2">客单价</th>
                </tr>
              </thead>
              <tbody>
                {donate.map((m) => (
                  <tr key={m.month} className="border-b border-line">
                    <td className="py-2">{m.month}</td>
                    <td className="py-2">{m.orders}</td>
                    <td className="py-2">{m.usd.toLocaleString()}</td>
                    <td className="py-2 text-sub">{m.avgUsd}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
