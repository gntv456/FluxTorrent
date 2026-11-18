"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 契约运营视图（0499，体检二轮 B1）：GET /admin/contract-stats。
 *  当月签约概览 + 近六月结算走势 + 档位分布——此前 admin 零读口，
 *  站长只能直连 DB 查「多少人签约/达标率/罚金总额」。 */

interface ContractStats {
  enabled: boolean;
  current: {
    open_seed: number;
    open_publish: number;
    eligible_users: number;
    seed_rate: number;
  };
  months: {
    month: string;
    settled: number;
    breached: number;
    reward: number;
    penalty: number;
  }[];
  tiers: { kind: string; target: number; signers: number }[];
}

export function AdminContractStats() {
  const [d, setD] = useState<ContractStats | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setD(await api.get<ContractStats>("/api/v1/admin/contract-stats"));
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : String(e));
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  if (err)
    return <p className="py-8 text-center text-[var(--danger-text)]">{err}</p>;
  if (!d) return <p className="py-8 text-center text-sub">…</p>;

  if (!d.enabled)
    return (
      <p className="brand-panel p-6 text-center text-sm text-sub">
        契约功能当前关闭（seed_contract_enabled）——开启后这里会出现签约
        与结算总览。
      </p>
    );

  return (
    <section className="space-y-4">
      <div className="grid gap-3 sm:grid-cols-4">
        {[
          ["当月在约 · 做种", d.current.open_seed.toLocaleString()],
          ["当月在约 · 发布", d.current.open_publish.toLocaleString()],
          [
            "签约率（做种）",
            `${d.current.seed_rate}% / ${d.current.eligible_users.toLocaleString()} 人`,
          ],
          [
            "近月罚金/奖励",
            (() => {
              const m = d.months[0];
              return m
                ? `${m.penalty.toLocaleString()} / ${m.reward.toLocaleString()}`
                : "—";
            })(),
          ],
        ].map(([label, value]) => (
          <div key={label} className="brand-panel p-3">
            <div className="text-xs text-sub">{label}</div>
            <div className="mt-1 font-display text-lg">{value}</div>
          </div>
        ))}
      </div>

      <div className="brand-panel p-4">
        <h2 className="mb-3 font-display text-lg">近六月结算走势</h2>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="py-2">月份</th>
              <th className="py-2">达标</th>
              <th className="py-2">未达标</th>
              <th className="py-2">达标率</th>
              <th className="py-2">奖励支出</th>
              <th className="py-2">罚金收入</th>
            </tr>
          </thead>
          <tbody>
            {d.months.length === 0 && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-sub">
                  尚无结算记录（首个结算在次月 3-4 日）
                </td>
              </tr>
            )}
            {d.months.map((m) => {
              const total = m.settled + m.breached;
              const rate =
                total > 0
                  ? Math.round((m.settled / total) * 100)
                  : 0;
              return (
                <tr key={m.month} className="border-b border-line/50">
                  <td className="py-2 font-mono text-xs">{m.month}</td>
                  <td className="py-2 text-up">{m.settled}</td>
                  <td className="py-2 text-down">{m.breached}</td>
                  <td className="py-2">{rate}%</td>
                  <td className="py-2 font-mono">
                    {m.reward.toLocaleString()}
                  </td>
                  <td className="py-2 font-mono">
                    {m.penalty.toLocaleString()}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div className="brand-panel p-4">
        <h2 className="mb-3 font-display text-lg">当月档位分布</h2>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="py-2">类型</th>
              <th className="py-2">档位目标</th>
              <th className="py-2">签约人数</th>
            </tr>
          </thead>
          <tbody>
            {d.tiers.length === 0 && (
              <tr>
                <td colSpan={3} className="py-6 text-center text-sub">
                  当月无人签约
                </td>
              </tr>
            )}
            {d.tiers.map((t) => (
              <tr
                key={`${t.kind}:${t.target}`}
                className="border-b border-line/50"
              >
                <td className="py-2">
                  {t.kind === "publish" ? "发布" : "做种"}
                </td>
                <td className="py-2 font-mono">
                  {t.kind === "publish"
                    ? `${t.target} 种`
                    : fmtSize(t.target)}
                </td>
                <td className="py-2">{t.signers}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function fmtSize(n: number): string {
  const gb = 1024 ** 3;
  if (n >= 1024 * gb) return `${(n / (1024 * gb)).toFixed(0)} TB`;
  if (n >= gb) return `${(n / gb).toFixed(0)} GB`;
  return `${n}`;
}
