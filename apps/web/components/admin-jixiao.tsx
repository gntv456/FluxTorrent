"use client";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 0106 绩效考核管理端补齐：
 *  岗位总览（本期登记/达标/发薪聚合 + 成员明细）
 *  + 批量分配（一个岗位一次登记多名用户，工作组口径）
 *  + 发薪记录（status=1 行 + 发放方式 worker/self） */

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

interface Payroll {
  period: string;
  total: number;
  list: PayrollRow[];
}

const STATUS_LABEL: Record<number, string> = {
  0: "进行中",
  1: "已发薪",
  2: "未达标",
  3: "已撤销",
};

export function AdminJixiao() {
  const { currency } = useI18n();
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
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
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
      flash("请选择岗位并填写用户 ID");
      return;
    }
    setBusy(true);
    try {
      const r = await api.post<{ assigned: number[]; skipped_dup: number[] }>(
        "/api/v1/admin/jixiao/assign-batch",
        { type_id: Number(form.type_id), user_ids: ids },
      );
      flash(
        `已分配 ${r.assigned.length} 人${r.skipped_dup.length ? `，跳过（本期已登记）${r.skipped_dup.length} 人` : ""}`,
      );
      setForm({ type_id: form.type_id, user_ids: "" });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "分配失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 font-mono text-sm outline-none focus:border-sky";
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
            ["overview", "考核总览"],
            ["assign", "批量分配"],
            ["payroll", "发薪记录"],
          ] as const
        ).map(([k, label]) => (
          <button
            key={k}
            className={`min-h-[32px] rounded-full px-4 text-xs font-bold ${tab === k ? "bg-sky text-cloud" : "border border-line"}`}
            onClick={() => setTab(k)}
          >
            {label}
          </button>
        ))}
        {ov && (
          <span className="ml-auto self-center text-xs text-sub">
            本期 {ov.period}
          </span>
        )}
      </div>

      {tab === "overview" && ov && (
        <>
          <table className="nexus-table text-xs">
            <thead>
              <tr>
                <td className="colhead">岗位</td>
                <td className="colhead">底薪</td>
                <td className="colhead">登记</td>
                <td className="colhead">达标</td>
                <td className="colhead">未达标</td>
                <td className="colhead">待结算</td>
                <td className="colhead">发薪总额</td>
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
                    暂无岗位
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          <table className="nexus-table text-xs">
            <thead>
              <tr>
                <td className="colhead">岗位</td>
                <td className="colhead">用户</td>
                <td className="colhead">状态</td>
                <td className="colhead">工资</td>
                <td className="colhead">加成</td>
                <td className="colhead">结算指标快照</td>
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
                  <td>{STATUS_LABEL[m.status] ?? m.status}</td>
                  <td className="num">{m.amount ?? "—"}</td>
                  <td className="num">{m.bonus ?? "—"}</td>
                  <td className="max-w-[320px] truncate font-mono text-sub">
                    {Object.keys(m.metrics_at_settle).length
                      ? JSON.stringify(m.metrics_at_settle)
                      : "未结算"}
                  </td>
                </tr>
              ))}
              {ov.members.length === 0 && (
                <tr>
                  <td colSpan={6} className="py-6 text-center text-sub">
                    本期暂无登记
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {tab === "assign" && (
        <section className="baozi-panel cmgmt-form p-4">
          <h2 className="mb-2 text-base font-bold">批量分配考核岗位</h2>
          <p className="mb-2 text-xs text-sub">
            把一个岗位登记给多名用户（工作组口径：主管建组）。已登记的自动跳过；分配即记录基线，月末由系统自动结算。
          </p>
          <div className="flex flex-wrap items-end gap-2">
            <label className="flex flex-col gap-1 text-xs">
              岗位
              <select
                value={form.type_id}
                onChange={(e) => setForm({ ...form, type_id: e.target.value })}
                className={`${inp.replace("font-mono ", "")} w-40`}
              >
                <option value="">— 选择 —</option>
                {types.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs">
              用户 ID（逗号/换行分隔）
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
              分配
            </button>
          </div>
        </section>
      )}

      {tab === "payroll" && pay && (
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
                    <a
                      className="text-sky underline"
                      href={`/users/${r.user_id}`}
                    >
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
      )}
    </div>
  );
}
