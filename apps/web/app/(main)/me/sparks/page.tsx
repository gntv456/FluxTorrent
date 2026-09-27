"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** C5 用户侧「我的魔力明细」页（0226，触点 #11）：
 *  消费既有 GET /me/spark/ledger（最近 50 条收支流水）。 */

interface LedgerRow {
  amount: number;
  kind: string;
  balance_after: number;
  created_at: string;
}

const KIND_LABELS: Record<string, string> = {
  shop: "商店消费",
  checkin: "签到",
  seeding: "做种收益",
  task: "任务奖励",
  forum: "论坛奖励",
  subtitle: "字幕奖励",
  games: "娱乐玩法",
  bank: "银行",
  pool: "站免池",
  admin: "管理调整",
  upload: "发布奖励",
  gift: "赠送",
};

export default function MySparksPage() {
  const { dict } = useI18n();
  void dict;
  const [rows, setRows] = useState<LedgerRow[] | null>(null);

  useEffect(() => {
    api
      .get<LedgerRow[]>("/api/v1/me/spark/ledger?limit=50")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);

  return (
    <div className="mx-auto max-w-3xl px-4 py-8">
      <h1 className="font-display text-xl">我的魔力明细</h1>
      <p className="mt-1 text-xs text-sub">
        最近 50 条收支（数据同账本流水，结算口径以各系统为准）。
      </p>
      <div className="baozi-panel mt-4 overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="px-3 py-2">时间</th>
              <th className="px-3 py-2">类型</th>
              <th className="px-3 py-2 text-right">变动</th>
              <th className="px-3 py-2 text-right">余额</th>
            </tr>
          </thead>
          <tbody>
            {(rows ?? []).length === 0 && (
              <tr>
                <td colSpan={4} className="py-8 text-center text-sub">
                  {rows === null ? "…" : "暂无流水"}
                </td>
              </tr>
            )}
            {(rows ?? []).map((r, i) => (
              <tr key={i} className="border-b border-line/50">
                <td className="px-3 py-2 font-mono text-xs">
                  {new Date(r.created_at).toLocaleString()}
                </td>
                <td className="px-3 py-2">
                  {KIND_LABELS[r.kind] ?? r.kind}
                </td>
                <td
                  className={`px-3 py-2 text-right font-mono ${
                    r.amount > 0 ? "text-up" : "text-down"
                  }`}
                >
                  {r.amount > 0 ? "+" : ""}
                  {r.amount.toLocaleString()}
                </td>
                <td className="px-3 py-2 text-right font-mono">
                  {r.balance_after.toLocaleString()}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
