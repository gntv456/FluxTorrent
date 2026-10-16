"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { SPARK_KIND_LABELS } from "@/lib/spark-kind-labels";
import { Pager } from "@/components/ui/pager";

/** C5 用户侧「我的魔力明细」页（0226，触点 #11）：
 *  消费 GET /me/spark/ledger 分页信封（2026-10-16 起：此前只有 limit 且
 *  写死 50 条，重度用户看不到更早的流水）。
 *  kind 人话映射与后台用户详情 SparkPanel 共享（lib/spark-kind-labels）。 */

/** 与后端 per 固定值一致（50/页） */
const PER = 50;

interface LedgerEnvelope {
  items: LedgerRow[];
  total: number;
  page: number;
  per: number;
}

interface LedgerRow {
  amount: number;
  kind: string;
  balance_after: number;
  created_at: string;
}

export default function MySparksPage() {
  const { dict } = useI18n();
  void dict;
  const [rows, setRows] = useState<LedgerRow[] | null>(null);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);

  const load = useCallback(() => {
    api
      .get<LedgerEnvelope>(`/api/v1/me/spark/ledger?page=${page}`)
      .then((d) => {
        setRows(d.items);
        setTotal(d.total);
      })
      .catch(() => setRows([]));
  }, [page]);
  useEffect(load, [load]);

  const pages = Math.ceil(total / PER);
  return (
    <div className="mx-auto max-w-3xl px-4 py-8">
      <h1 className="font-display text-xl">我的魔力明细</h1>
      <p className="mt-1 text-xs text-sub">
        共 {total} 条收支（数据同账本流水，结算口径以各系统为准）。
      </p>
      <div className="brand-panel mt-4 overflow-x-auto">
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
                  {SPARK_KIND_LABELS[r.kind] ?? r.kind}
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
      {pages > 1 && (
        <Pager
          layout="compact"
          page={page}
          pages={pages}
          posLabel={`${page} / ${pages}`}
          onStep={setPage}
        />
      )}
    </div>
  );
}
