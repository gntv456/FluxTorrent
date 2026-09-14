"use client";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P3-13：考核岗位类型配置（好学站 system/exams 简化口径）
 *  jixiao_types CRUD：指标/底薪/门槛/加成规则（JSONB） */

interface JixiaoTypeRow {
  id: number;
  name: string;
  metrics: Record<string, unknown>;
  base_pay: number;
  min_requirements: Record<string, unknown>;
  bonus_rules: Record<string, unknown>;
}

const EMPTY = { name: "", metrics: "{}", base_pay: "0", min_requirements: "{}", bonus_rules: "{}" };

export function AdminExams() {
  const { currency } = useI18n();
  const [rows, setRows] = useState<JixiaoTypeRow[]>([]);
  const [edit, setEdit] = useState<{ id: number | null; f: typeof EMPTY }>({ id: null, f: { ...EMPTY } });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => { setMsg(m); setTimeout(() => setMsg(null), 3000); };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<JixiaoTypeRow[]>("/api/v1/admin/jixiao-types"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, []);
  useEffect(() => { load(); }, [load]);

  async function save() {
    let metrics: unknown, reqs: unknown, rules: unknown;
    try {
      metrics = JSON.parse(edit.f.metrics || "{}");
      reqs = JSON.parse(edit.f.min_requirements || "{}");
      rules = JSON.parse(edit.f.bonus_rules || "{}");
    } catch { flash("JSON 格式有误"); return; }
    setBusy(true);
    try {
      const payload = { name: edit.f.name, metrics, base_pay: Number(edit.f.base_pay) || 0, min_requirements: reqs, bonus_rules: rules };
      if (edit.id === null) await api.post("/api/v1/admin/jixiao-types", payload);
      else await api.put(`/api/v1/admin/jixiao-types/${edit.id}`, payload);
      flash("已保存");
      setEdit({ id: null, f: { ...EMPTY } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp = "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 font-mono text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">{edit.id === null ? "新建考核岗位" : `编辑考核岗位 #${edit.id}`}</h2>
        <p className="mb-2 text-xs text-sub">登记入口在用户详情页「分配考核」；这里维护岗位类型、核算指标与奖金规则。</p>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">岗位名
            <input value={edit.f.name} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })} className={`${inp.replace("font-mono ", "")} w-36`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">底薪({currency})
            <input type="number" value={edit.f.base_pay} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, base_pay: e.target.value } })} className={`${inp.replace("font-mono ", "")} w-24`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">指标 metrics
            <input value={edit.f.metrics} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, metrics: e.target.value } })} placeholder='{"seed_hours":100}' className={`${inp} w-56`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">门槛 min_requirements
            <input value={edit.f.min_requirements} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, min_requirements: e.target.value } })} className={`${inp} w-56`} />
          </label>
          <label className="flex flex-col gap-1 text-xs">奖金规则 bonus_rules
            <input value={edit.f.bonus_rules} onChange={(e) => setEdit({ ...edit, f: { ...edit.f, bonus_rules: e.target.value } })} className={`${inp} w-56`} />
          </label>
          <button className="baozi-button" disabled={busy || !edit.f.name.trim()} onClick={save}>保存</button>
          {edit.id !== null && <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setEdit({ id: null, f: { ...EMPTY } })}>取消</button>}
        </div>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td><td className="colhead">岗位</td><td className="colhead">底薪</td>
            <td className="colhead">指标</td><td className="colhead">登记数</td><td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td className="num">{r.id}</td>
              <td className="font-bold">{r.name}</td>
              <td className="num">{r.base_pay}</td>
              <td className="max-w-[280px] truncate font-mono">{JSON.stringify(r.metrics)}</td>
              <td className="num">{/* 登记数在 claims 表，列表接口不联查，展示 — */}—</td>
              <td className="text-right">
                <button className="cmgmt-act" onClick={() => setEdit({ id: r.id, f: { name: r.name, metrics: JSON.stringify(r.metrics), base_pay: String(r.base_pay), min_requirements: JSON.stringify(r.min_requirements), bonus_rules: JSON.stringify(r.bonus_rules) } })}>编辑</button>
                <button className="cmgmt-act cmgmt-act--danger" disabled={busy}
                  onClick={async () => {
                    try { await api.del(`/api/v1/admin/jixiao-types/${r.id}`); flash("已删除"); await load(); }
                    catch (e) { flash(e instanceof ApiError ? e.message : "删除失败"); }
                  }}>删除</button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && <tr><td colSpan={6} className="py-6 text-center text-sub">暂无考核岗位</td></tr>}
        </tbody>
      </table>
    </div>
  );
}
