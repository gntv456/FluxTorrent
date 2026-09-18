"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface SparkInfo {
  balance: number;
  seeding_count: number;
  hourly_estimate: number;
}

interface LedgerRow {
  amount: number;
  kind: string;
  balance_after: number | null;
  created_at: string;
}

const KIND_LABEL: Record<string, string> = {
  seeding: "做种收益",
  download: "下载消耗",
  shop: "商店消费",
  task: "任务奖励",
  attendance: "签到",
  bank: "银行",
  subtitle: "字幕奖励",
  forum: "论坛奖励",
  admin: "管理员发放",
  grant: "发放",
  hourly: "小时结算",
};

/** 我的魔力（mybonus.php 口径）：余额卡片 + 收益估算 + 最近流水 */
export default function MySparkPage() {
  const { dict, locale, currency } = useI18n();
  const t = dict.myspark;
  const [info, setInfo] = useState<SparkInfo | null>(null);
  const [rows, setRows] = useState<LedgerRow[]>([]);
  const [limit, setLimit] = useState(20);
  const [err, setErr] = useState<string | null>(null);
  const loggedIn = typeof window !== "undefined" && !!localStorage.getItem("flux.token");

  const load = useCallback(() => {
    api.get<SparkInfo>("/api/v1/me/spark").then(setInfo).catch(() => setInfo(null));
    api.get<LedgerRow[]>(`/api/v1/me/spark/ledger?limit=${limit}`)
      .then(setRows)
      .catch((e) => {
        setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
        setRows([]);
      });
  }, [limit, dict]);
  useEffect(load, [load]);

  if (!loggedIn) {
    return (
      <div className="flex flex-col gap-4">
        <h1 className="font-display text-2xl">{t.title.replace("{magic}", currency)}</h1>
        <p className="baozi-panel p-4 text-sm text-sub">{dict.common.pleaseLogin}</p>
      </div>
    );
  }

  const income = rows.filter((r) => r.amount > 0).reduce((s, r) => s + r.amount, 0);
  const spend = rows.filter((r) => r.amount < 0).reduce((s, r) => s + r.amount, 0);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{t.title.replace("{magic}", currency)}</h1>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <div className="baozi-panel p-4">
          <p className="text-xs text-sub">{t.balance}</p>
          <p className="num text-2xl font-bold text-[var(--baozi-orange-dark)]">
            ✨ {info ? info.balance.toLocaleString() : "…"}
          </p>
        </div>
        <div className="baozi-panel p-4">
          <p className="text-xs text-sub">{t.hourly}</p>
          <p className="num text-2xl font-bold text-ink">
            {info ? `+${info.hourly_estimate}` : "…"}
          </p>
        </div>
        <div className="baozi-panel p-4">
          <p className="text-xs text-sub">{t.seedingCount}</p>
          <p className="num text-2xl font-bold text-ink">{info ? info.seeding_count : "…"}</p>
        </div>
        <div className="baozi-panel p-4">
          <p className="text-xs text-sub">{t.ruleTitle}</p>
          <p className="text-xs leading-relaxed text-sub">{t.rule.replace("{magic}", currency)}</p>
        </div>
      </div>

      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead" colSpan={4}>
              <div className="flex items-baseline justify-between">
                <h2 className="font-display">{t.ledger.replace("{magic}", currency)}</h2>
                <span className="text-xs font-normal text-sub">
                  {t.recentIncome} <b className="num text-success">+{income.toLocaleString()}</b> ·{" "}
                  {t.recentSpend} <b className="num text-danger">{spend.toLocaleString()}</b>
                </span>
              </div>
            </td>
          </tr>
          <tr>
            <td className="colhead">{t.ledgerKind}</td>
            <td className="colhead">{t.ledgerAmount}</td>
            <td className="colhead">{t.ledgerBalance}</td>
            <td className="colhead">{t.ledgerAt}</td>
          </tr>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>{KIND_LABEL[r.kind] ?? r.kind}</td>
              <td className={`num font-bold ${r.amount >= 0 ? "text-success" : "text-danger"}`}>
                {r.amount >= 0 ? "+" : ""}{r.amount.toLocaleString()}
              </td>
              <td className="num">{r.balance_after?.toLocaleString() ?? "—"}</td>
              <td className="text-xs text-sub">
                {new Date(r.created_at).toLocaleString(dateLocale(locale))}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={4} className="py-6 text-center text-sub">
                {err ?? t.ledgerEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>

      {rows.length >= limit && (
        <button
          className="self-center min-h-[40px] rounded-full border border-line px-6 text-sm font-bold"
          onClick={() => setLimit((n) => Math.min(n + 20, 50))}
        >
          {t.loadMore}
        </button>
      )}
    </div>
  );
}
