"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 0106 绩效考核管理端·发薪记录（从 components/admin-jixiao.tsx 按域拆出）：
 *  status=1 行 + 发放方式 worker/self（月末自动结算 / 本人领取）。 */

interface PayrollRow {
  claim_id: number;
  user_id: number;
  username: string;
  type_name: string;
  amount: number;
  bonus: number;
  paid_by: string;
  settled_at: string;
}

export interface Payroll {
  period: string;
  total: number;
  list: PayrollRow[];
}

export function PayrollTab({ pay }: { pay: Payroll }) {
  const { currency, dict, locale } = useI18n();
  const at = dict.adminJixiao;
  return (
    <>
      <p className="text-xs text-sub">
        {at.payrollTotalLabel}
        <strong className="text-ink">
          {pay.total.toLocaleString()} {currency}
        </strong>
        {at.paidNote}
      </p>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thClaimId}</td>
            <td className="colhead">{at.thPayrollUser}</td>
            <td className="colhead">{at.thPayrollType}</td>
            <td className="colhead">{at.thPay}</td>
            <td className="colhead">{at.thBonus}</td>
            <td className="colhead">{at.thWay}</td>
            <td className="colhead">{at.thPayTime}</td>
          </tr>
        </thead>
        <tbody>
          {pay.list.map((r) => (
            <tr key={r.claim_id}>
              <td className="num">{r.claim_id}</td>
              <td>
                <a className="text-sky underline" href={`/users/${r.user_id}`}>
                  {r.username}
                </a>
              </td>
              <td>{r.type_name}</td>
              <td className="num">{r.amount.toLocaleString()}</td>
              <td className="num">{r.bonus.toLocaleString()}</td>
              <td>{r.paid_by}</td>
              <td className="text-sub">
                {new Date(r.settled_at).toLocaleString(dateLocale(locale))}
              </td>
            </tr>
          ))}
          {pay.list.length === 0 && (
            <tr>
              <td colSpan={7} className="py-6 text-center text-sub">
                {at.payrollEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </>
  );
}
