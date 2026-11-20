"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 火花月度对账（0507，从无前端到可见）：GET /admin/spark-flow 的
 *  v_spark_flow_monthly 24 月产出/回收/净增发——通胀监控的数据底座
 *  此前只有 API 没有任何组件消费，站长看不到该面板。挂经济运营面板
 *  （AdminEconomyDashboard）底部。 */

interface SparkFlowMonth {
  month: string;
  minted: number;
  burned: number;
  net: number;
  entries: number;
}

export function SparkFlowTable() {
  const [rows, setRows] = useState<SparkFlowMonth[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const raw = await api.get<
        ([string, number, number, number, number] | SparkFlowMonth)[]
      >("/api/v1/admin/spark-flow");
      // API 返回元组数组（month, minted, burned, net, entries）
      setRows(
        (raw ?? []).map((r) =>
          Array.isArray(r)
            ? {
                month: r[0],
                minted: r[1],
                burned: r[2],
                net: r[3],
                entries: r[4],
              }
            : r,
        ),
      );
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (err)
    return <p className="py-4 text-center text-sub">{err}</p>;
  if (!rows) return <p className="py-4 text-center text-sub">…</p>;
  if (rows.length === 0)
    return <p className="py-4 text-center text-sub">暂无对账数据</p>;

  return (
    <div className="brand-panel p-4">
      <h2 className="mb-3 font-display text-lg">火花月度对账（24 月）</h2>
      <div className="brand-wide-table-scroll">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="py-2">月份</th>
              <th className="py-2">产出</th>
              <th className="py-2">回收</th>
              <th className="py-2">净增发</th>
              <th className="py-2">流水中</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((m) => (
              <tr key={m.month} className="border-b border-line">
                <td className="py-2">{m.month}</td>
                <td className="py-2">{m.minted.toLocaleString()}</td>
                <td className="py-2">{m.burned.toLocaleString()}</td>
                <td
                  className={`py-2 ${
                    m.net > 0 ? "text-[var(--danger-text)]" : "text-sub"
                  }`}
                >
                  {m.net > 0 ? "+" : ""}
                  {m.net.toLocaleString()}
                </td>
                <td className="py-2 text-sub">{m.entries.toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
