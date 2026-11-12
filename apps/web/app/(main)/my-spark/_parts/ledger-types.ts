/** 流水行/合计的形状与派生（2026-11-13 标签化批随 LedgerPanel 拆出；
 *  page.tsx 与 ledger-panel.tsx 共用，信源是 /me/spark/ledger） */

export interface LedgerRow {
  amount: number;
  kind: string;
  balance_after: number | null;
  created_at: string;
  /** 商店消费带的商品名（其它流水为 null）——kind 全是 'shop' 分不清 */
  item_name?: string | null;
}

export interface LedgerEnvelope {
  items: LedgerRow[];
  total: number;
  page: number;
  per: number;
}

export type Summary = { income: number; spend: number };

/** 与后端 /me/spark/ledger 的 per 固定值一致（50/页） */
export const LEDGER_PER = 50;

export const sums = (rows: LedgerRow[]): Summary => ({
  income: rows.filter((r) => r.amount > 0).reduce((s, r) => s + r.amount, 0),
  spend: rows.filter((r) => r.amount < 0).reduce((s, r) => s + r.amount, 0),
});
