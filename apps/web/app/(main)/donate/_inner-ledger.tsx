"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 捐赠中心展示件（从 donate/_inner.tsx 按域拆出）：
 *  储值流水表格（充值/订购两类，数据装载留在 _inner.tsx）。 */

export interface LedgerRow {
  id: number;
  kind: "topup" | "order";
  amount_usd: number;
  balance_after: number;
  note: string | null;
  created_at: string;
}

/** 储值流水：充值/订购两类 + 余额快照 + 时间（空时给占位行） */
export function LedgerTable({ rows }: { rows: LedgerRow[] | undefined }) {
  const { dict, locale } = useI18n();
  const t = dict.donate;
  return (
    <table className="nexus-table">
      <tbody>
        <tr>
          <td className="colhead" colSpan={4}>
            <h2 className="font-display">{t.ledgerTitle}</h2>
          </td>
        </tr>
        <tr>
          <td className="colhead">{t.ledgerKind}</td>
          <td className="colhead">{t.ledgerAmount}</td>
          <td className="colhead">{t.balance}</td>
          <td className="colhead">{t.ledgerAt}</td>
        </tr>
        {(rows ?? []).map((r) => (
          <tr key={r.id}>
            <td>{r.kind === "topup" ? t.kindTopup : t.kindOrder}{r.note ? ` · ${r.note}` : ""}</td>
            <td className={`num font-bold ${r.amount_usd >= 0 ? "text-success" : "text-danger"}`}>
              {r.amount_usd >= 0 ? "+" : ""}{r.amount_usd.toFixed(2)}
            </td>
            <td className="num">{r.balance_after.toFixed(2)}</td>
            <td className="text-xs text-sub">{new Date(r.created_at).toLocaleString(dateLocale(locale))}</td>
          </tr>
        ))}
        {(!rows || rows.length === 0) && (
          <tr><td colSpan={4} className="py-6 text-center text-sub">{t.ledgerEmpty}</td></tr>
        )}
      </tbody>
    </table>
  );
}
