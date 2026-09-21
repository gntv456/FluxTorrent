"use client";

import { useI18n } from "@/i18n/client";

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
  const { currency } = useI18n();
  return (
    <>
      <p className="text-xs text-sub">
        本期发薪总额：
        <strong className="text-ink">
          {pay.total.toLocaleString()} {currency}
        </strong>
        （paid_by：worker=月末自动结算，self=本人领取）
      </p>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">领取ID</td>
            <td className="colhead">用户</td>
            <td className="colhead">岗位</td>
            <td className="colhead">工资</td>
            <td className="colhead">加成</td>
            <td className="colhead">方式</td>
            <td className="colhead">时间</td>
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
                {new Date(r.settled_at).toLocaleString()}
              </td>
            </tr>
          ))}
          {pay.list.length === 0 && (
            <tr>
              <td colSpan={7} className="py-6 text-center text-sub">
                本期暂无发薪
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </>
  );
}
