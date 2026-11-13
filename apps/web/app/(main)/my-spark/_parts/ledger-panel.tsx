"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { StatePanel } from "@/components/ui/state-panel";
import { SkeletonRows } from "@/components/ui/skeleton";
import { Pager } from "@/components/ui/pager";
import { LEDGER_PER } from "./ledger-types";
import type { LedgerRow, Summary } from "./ledger-types";

/** {magic}流水面板（2026-11-13 标签化批从 page.tsx 拆出，300 行门禁）：
 *  表 + 真分页（2026-10-16 改真分页：旧「加载更多」封顶 50 条且无提示）。
 *  近期收支合计由父级传入（固定第一页口径，翻页只换表格不换合计）。
 *  M6.5：compact 档 CSS 双态（.profilet 同款：thead 隐藏+块化+小标签），
 *  客户端组件但免 useIsCompact 双树——流水 4 列窄屏改行读。 */
export function LedgerPanel({
  rows,
  total,
  page,
  setPage,
  summary,
  err,
  onRetry,
}: {
  rows: LedgerRow[];
  total: number;
  page: number;
  setPage: (p: number) => void;
  summary: Summary | null;
  err: string | null;
  onRetry: () => void;
}) {
  const { dict, locale, currency } = useI18n();
  const t = dict.myspark;
  const income = summary?.income;
  const spend = summary?.spend;
  return (
    <>
      <div className="brand-panel">
        <div className="brand-panel__head">
          <h2>{t.ledger.replace("{magic}", currency)}</h2>
          <span className="text-xs text-sub">
            {t.recentIncome}{" "}
            <b className="num text-success">
              {income !== undefined ? `+${income.toLocaleString()}` : "—"}
            </b>
            {" · "}
            {t.recentSpend}{" "}
            <b className="num text-[var(--danger-text)]">
              {spend !== undefined ? spend.toLocaleString() : "—"}
            </b>
          </span>
        </div>
        {/* A3 四态：流水表 loading 骨架 / 错误占位 / 空态走 StatePanel */}
        <StatePanel
          loading={rows.length === 0 && err === null}
          error={rows.length === 0 ? err : null}
          empty={rows.length === 0}
          loadingFallback={<SkeletonRows rows={5} />}
          onRetry={onRetry}
        >
          <div className="brand-wide-table-scroll">
            <table className="nexus-table profilet">
              <thead>
        <tr>
        <th scope="col" className="colhead">{t.ledgerKind}</th>
        <th scope="col" className="colhead">{t.ledgerAmount}</th>
        <th scope="col" className="colhead">{t.ledgerBalance}</th>
        <th scope="col" className="colhead">{t.ledgerAt}</th>
        </tr>
        </thead>
      <tbody>
                {rows.map((r, i) => (
                  <tr key={i}>
                    <td>
                      <span className="profilet__label">{t.ledgerKind}</span>
                      <span className="pill">
                        {r.item_name
                          ? r.item_name
                          : (dict.my.kinds[r.kind] ?? t.otherKind)}
                      </span>
                    </td>
                    <td
                      className={`num font-bold ${
                        r.amount >= 0
                          ? "text-success"
                          : "text-[var(--danger-text)]"
                      }`}
                    >
                      <span className="profilet__label">{t.ledgerAmount}</span>
                      {r.amount >= 0 ? "+" : ""}
                      {r.amount.toLocaleString()}
                    </td>
                    <td className="num">
                      <span className="profilet__label">{t.ledgerBalance}</span>
                      {r.balance_after?.toLocaleString() ?? "—"}
                    </td>
                    <td className="text-xs text-sub">
                      <span className="profilet__label">{t.ledgerAt}</span>
                      {new Date(r.created_at).toLocaleString(
                        dateLocale(locale),
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </StatePanel>
      </div>

      {total > LEDGER_PER && (
        <Pager
          layout="compact"
          page={page}
          pages={Math.ceil(total / LEDGER_PER)}
          posLabel={`${page} / ${Math.ceil(total / LEDGER_PER)}`}
          onStep={setPage}
        />
      )}
    </>
  );
}
