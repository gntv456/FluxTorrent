"use client";

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

import { Payroll, PayrollTab } from "./admin-jixiao-payroll";

/** 0106 绩效考核管理端补齐：
 *  岗位总览（本期登记/达标/发薪聚合 + 成员明细）
 *  + 批量分配（一个岗位一次登记多名用户，工作组口径）
 *  + 发薪记录（status=1 行 + 发放方式 worker/self）。
 *  发薪记录 tab 拆至 ./admin-jixiao-payroll.tsx。 */

interface OverviewType {
  type_id: number;
  name: string;
  base_pay: number;
  assigned: number;
  qualified: number;
  failed: number;
  pending: number;
  payroll_total: number;
}

interface OverviewMember {
  type_id: number;
  user_id: number;
  username: string;
  status: number; // 0=进行中 1=达标已发薪 2=未达标 3=撤销
  amount: number | null;
  bonus: number | null;
  metrics_at_settle: Record<string, number>;
}

interface Overview {
  period: string;
  types: OverviewType[];
  members: OverviewMember[];
}

export function AdminJixiao() {
  const { dict } = useI18n();
  const at = dict.adminJixiao;
  const [tab, setTab] = useState<"overview" | "assign" | "payroll">("overview");
  const [ov, setOv] = useState<Overview | null>(null);
  const [pay, setPay] = useState<Payroll | null>(null);
  const [types, setTypes] = useState<{ id: number; name: string }[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [form, setForm] = useState({ type_id: "", user_ids: "" });

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3500);
  };

  const load = useCallback(async () => {
    try {
      const [o, p, t] = await Promise.all([
        api.get<Overview>("/api/v1/admin/jixiao/overview"),
        api.get<Payroll>("/api/v1/admin/jixiao/payroll"),
        api.get<{ id: number; name: string }[]>("/api/v1/admin/jixiao-types"),
      ]);
      setOv(o);
      setPay(p);
      setTypes(t);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.loadFail);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function assignBatch() {
    const ids = form.user_ids
      .split(/[\s,;，；]+/)
      .map((s) => Number(s.trim()))
      .filter((n) => n > 0);
    if (!form.type_id || ids.length === 0) {
      flash(at.pickTypeAndUsers);
      return;
    }
    setBusy(true);
    try {
      const r = await api.post<{ assigned: number[]; skipped_dup: number[] }>(
        "/api/v1/admin/jixiao/assign-batch",
        { type_id: Number(form.type_id), user_ids: ids },
      );
      const skip = r.skipped_dup.length
        ? fmt(at.skippedDup, { n: r.skipped_dup.length })
        : "";
      flash(fmt(at.assignedMsg, { n: r.assigned.length }) + skip);
      setForm({ type_id: form.type_id, user_ids: "" });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.assignFail);
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
    "bg-cloud px-2 font-mono text-sm outline-none focus:border-sky";
  const typeName = (tid: number) =>
    types.find((t) => t.id === tid)?.name ?? `#${tid}`;

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <div className="flex gap-2">
        {(
          [
            ["overview", at.tabOverview],
            ["assign", at.tabAssign],
            ["payroll", at.tabPayroll],
          ] as const
        ).map(([k, label]) => (
          <button
            key={k}
            className={
              "min-h-[32px] rounded-full px-4 text-xs font-bold " +
              `${tab === k ? "bg-sky text-cloud" : "border border-line"}`
            }
            onClick={() => setTab(k)}
          >
            {label}
          </button>
        ))}
        {ov && (
          <span className="ml-auto self-center text-xs text-sub">
            {fmt(at.periodLabel, { period: ov.period })}
          </span>
        )}
      </div>

      {tab === "overview" && ov && (
        <>
          <table className="nexus-table text-xs">
            <thead>
              <tr>
                <td className="colhead">{at.thType}</td>
                <td className="colhead">{at.thBasePay}</td>
                <td className="colhead">{at.thAssigned}</td>
                <td className="colhead">{at.thQualified}</td>
                <td className="colhead">{at.thFailed}</td>
                <td className="colhead">{at.thPending}</td>
                <td className="colhead">{at.thPayrollTotal}</td>
              </tr>
            </thead>
            <tbody>
              {ov.types.map((t) => (
                <tr key={t.type_id}>
                  <td className="font-bold">{t.name}</td>
                  <td className="num">{t.base_pay}</td>
                  <td className="num">{t.assigned}</td>
                  <td className="num">{t.qualified}</td>
                  <td className="num">{t.failed}</td>
                  <td className="num">{t.pending}</td>
                  <td className="num">{t.payroll_total}</td>
                </tr>
              ))}
              {ov.types.length === 0 && (
                <tr>
                  <td colSpan={7} className="py-6 text-center text-sub">
                    {at.typesEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          <table className="nexus-table text-xs">
            <thead>
              <tr>
                <td className="colhead">{at.thType}</td>
                <td className="colhead">{at.thUser}</td>
                <td className="colhead">{at.thStatus}</td>
                <td className="colhead">{at.thPay}</td>
                <td className="colhead">{at.thBonus}</td>
                <td className="colhead">{at.thMetrics}</td>
              </tr>
            </thead>
            <tbody>
              {ov.members.map((m, i) => (
                <tr key={i}>
                  <td>{typeName(m.type_id)}</td>
                  <td>
                    <a
                      className="text-sky underline"
                      href={`/users/${m.user_id}`}
                    >
                      {m.username}
                    </a>
                  </td>
                  <td>
                    {at.statusLabels[String(m.status)] ?? m.status}
                  </td>
                  <td className="num">{m.amount ?? "—"}</td>
                  <td className="num">{m.bonus ?? "—"}</td>
                  <td className="max-w-[320px] truncate font-mono text-sub">
                    {Object.keys(m.metrics_at_settle).length
                      ? JSON.stringify(m.metrics_at_settle)
                      : at.notSettled}
                  </td>
                </tr>
              ))}
              {ov.members.length === 0 && (
                <tr>
                  <td colSpan={6} className="py-6 text-center text-sub">
                    {at.membersEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {tab === "assign" && (
        <section className="baozi-panel cmgmt-form p-4">
          <h2 className="mb-2 text-base font-bold">{at.assignTitle}</h2>
          <p className="mb-2 text-xs text-sub">{at.assignHint}</p>
          <div className="flex flex-wrap items-end gap-2">
            <label className="flex flex-col gap-1 text-xs">
              {at.labelType}
              <select
                value={form.type_id}
                onChange={(e) => setForm({ ...form, type_id: e.target.value })}
                className={`${inp.replace("font-mono ", "")} w-40`}
              >
                <option value="">{at.selectOne}</option>
                {types.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {at.labelUserIds}
              <input
                value={form.user_ids}
                onChange={(e) => setForm({ ...form, user_ids: e.target.value })}
                placeholder="3, 5, 8"
                className={`${inp} w-72`}
              />
            </label>
            <button
              className="baozi-button"
              disabled={busy}
              onClick={assignBatch}
            >
              {at.assign}
            </button>
          </div>
        </section>
      )}

      {tab === "payroll" && pay && <PayrollTab pay={pay} />}
    </div>
  );
}
