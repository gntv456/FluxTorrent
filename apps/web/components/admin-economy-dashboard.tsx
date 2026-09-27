"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** C5 经济反通胀运营面板（0226）：GET /admin/economy-dashboard。
 *  近 8 周产出/消耗/净增发 + 池子存量 + Top 消耗 SKU + 人均持币 + 阀门现值。
 *  数据全部来自 spark_ledger 既有流水。 */

interface Dashboard {
  weeks: { week: string; minted: number; burned: number; net: number }[];
  pool_balance: number;
  total_spark: number;
  holders: number;
  active_30d: number;
  per_active_capita: number;
  top_sku_30d: { name: string; orders: number; spent: number }[];
  purchase_cap_gb: number;
}

export function AdminEconomyDashboard() {
  const [d, setD] = useState<Dashboard | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setD(await api.get<Dashboard>("/api/v1/admin/economy-dashboard"));
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : String(e));
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  if (err)
    return <p className="py-8 text-center text-danger">{err}</p>;
  if (!d) return <p className="py-8 text-center text-sub">…</p>;

  const maxAbs = Math.max(
    1,
    ...d.weeks.flatMap((w) => [w.minted, w.burned]),
  );

  return (
    <section className="space-y-4">
      <div className="grid gap-3 sm:grid-cols-4">
        {[
          ["全站持币", d.total_spark.toLocaleString()],
          ["站免池存量", d.pool_balance.toLocaleString()],
          [
            "活跃人均（30 天）",
            `${d.per_active_capita.toLocaleString()} / `
              + `${d.active_30d.toLocaleString()} 人`,
          ],
          [
            "购买上限阀门",
            d.purchase_cap_gb > 0
              ? `${d.purchase_cap_gb} GB`
              : "未启用（0）",
          ],
        ].map(([label, value]) => (
          <div key={label} className="baozi-panel p-3">
            <div className="text-xs text-sub">{label}</div>
            <div className="mt-1 font-display text-lg">{value}</div>
          </div>
        ))}
      </div>

      <div className="baozi-panel p-4">
        <h2 className="mb-3 font-display text-lg">
          火花产出 / 消耗（近 8 周）
        </h2>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="py-2">周起点</th>
              <th className="py-2">产出</th>
              <th className="py-2">消耗</th>
              <th className="py-2">净增发</th>
              <th className="py-2 w-1/3">比例</th>
            </tr>
          </thead>
          <tbody>
            {d.weeks.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  暂无流水
                </td>
              </tr>
            )}
            {d.weeks.map((w) => (
              <tr key={w.week} className="border-b border-line/50">
                <td className="py-2 font-mono text-xs">{w.week}</td>
                <td className="py-2 text-up">{w.minted.toLocaleString()}</td>
                <td className="py-2 text-down">{w.burned.toLocaleString()}</td>
                <td
                  className={`py-2 ${w.net > 0 ? "text-up" : "text-down"}`}
                >
                  {w.net.toLocaleString()}
                </td>
                <td className="py-2">
                  <div className="flex h-3 items-center gap-1">
                    <div
                      className="h-2 rounded bg-up/70"
                      style={{
                        width: `${(w.minted / maxAbs) * 100}%`,
                      }}
                    />
                    <div
                      className="h-2 rounded bg-down/70"
                      style={{
                        width: `${(w.burned / maxAbs) * 100}%`,
                      }}
                    />
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="baozi-panel p-4">
        <h2 className="mb-3 font-display text-lg">
          Top 消耗 SKU（近 30 天）
        </h2>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="py-2">商品</th>
              <th className="py-2">订单数</th>
              <th className="py-2">消耗火花</th>
            </tr>
          </thead>
          <tbody>
            {d.top_sku_30d.length === 0 && (
              <tr>
                <td colSpan={3} className="py-6 text-center text-sub">
                  暂无订单
                </td>
              </tr>
            )}
            {d.top_sku_30d.map((s) => (
              <tr key={s.name} className="border-b border-line/50">
                <td className="py-2">{s.name}</td>
                <td className="py-2">{s.orders.toLocaleString()}</td>
                <td className="py-2 font-mono">{s.spent.toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
