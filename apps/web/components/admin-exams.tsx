"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { ExamsTable } from "./admin-exams-table";
import type { JixiaoTypeRow } from "./admin-exams-shared";
import {
  EMPTY_FORM,
  jsonToReqs,
  metricOptions,
  reqsToJson,
} from "./admin-exams-shared";

/** 第八轮 P3-13 + 0106 结构化改版：考核岗位类型配置
 *  指标门槛不再手写 JSON —— 下拉选指标 + 填数值（可多行 AND 组合）；
 *  加成规则也不再写代码：选档位数（每 N 个达标月）和加成百分比。
 *  底层仍存 min_requirements / bonus_rules JSONB，兼容旧数据。
 *  岗位表拆至 ./admin-exams-table.tsx；指标目录与换算拆至
 *  ./admin-exams-shared.ts。 */

/** 「+ 添加指标」小按钮 */
const ADD_METRIC_BTN_CLS =
  "mt-2 min-h-[32px] rounded-full border border-line px-4 text-xs font-bold";

export function AdminExams() {
  const { currency } = useI18n();
  const METRIC_OPTIONS = metricOptions(currency);
  const [rows, setRows] = useState<JixiaoTypeRow[]>([]);
  const [edit, setEdit] = useState<{ id: number | null; f: typeof EMPTY_FORM }>(
    { id: null, f: { ...EMPTY_FORM, reqs: [...EMPTY_FORM.reqs] } },
  );
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3500);
  };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<JixiaoTypeRow[]>("/api/v1/admin/jixiao-types"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  function startEdit(r: JixiaoTypeRow) {
    const rules = r.bonus_rules as {
      months_per_step?: number;
      percent_per_step?: number;
    };
    setEdit({
      id: r.id,
      f: {
        name: r.name,
        base_pay: String(r.base_pay),
        description: r.description ?? "",
        reqs: jsonToReqs(r.min_requirements ?? {}),
        bonusStep: String(rules.months_per_step ?? 3),
        bonusPct: String(rules.percent_per_step ?? 10),
      },
    });
  }

  async function save() {
    setBusy(true);
    try {
      const payload = {
        name: edit.f.name,
        metrics: {},
        base_pay: Number(edit.f.base_pay) || 0,
        min_requirements: reqsToJson(edit.f.reqs),
        // bonus_rules 结构化落库（当前结算走 site_settings 全局配置，
        // 此处存岗位级意图，后续按岗位覆盖时直接可用）
        bonus_rules: {
          months_per_step: Number(edit.f.bonusStep) || 3,
          percent_per_step: Number(edit.f.bonusPct) || 0,
        },
        description: edit.f.description,
      };
      if (edit.id === null)
        await api.post("/api/v1/admin/jixiao-types", payload);
      else await api.put(`/api/v1/admin/jixiao-types/${edit.id}`, payload);
      flash("已保存");
      setEdit({ id: null, f: { ...EMPTY_FORM, reqs: [...EMPTY_FORM.reqs] } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud " +
    "px-2 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {edit.id === null ? "新建考核岗位" : `编辑考核岗位 #${edit.id}`}
        </h2>
        <p className="mb-3 text-xs text-sub">
          登记入口在「绩效考核」面板批量分配，或用户详情页单人分配；这里维护岗位、指标门槛与加成。
        </p>

        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">
            岗位名
            <input
              value={edit.f.name}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
              }
              className={`${inp} w-36`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            底薪({currency})
            <input
              type="number"
              value={edit.f.base_pay}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, base_pay: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            岗位说明
            <input
              value={edit.f.description}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, description: e.target.value },
                })
              }
              placeholder="展示给成员的岗位职责（可空）"
              className={`${inp} w-56`}
            />
          </label>
        </div>

        {/* 达标门槛：结构化行（指标下拉 + 数值），多行之间是 AND */}
        <div className="mt-3">
          <p className="mb-1 text-xs font-bold">达标门槛（全部满足才算达标）</p>
          <div className="flex flex-col gap-2">
            {edit.f.reqs.map((r, i) => {
              const opt = METRIC_OPTIONS.find((m) => m.key === r.key);
              return (
                <div key={i} className="flex flex-wrap items-center gap-2">
                  <select
                    value={r.key}
                    onChange={(e) => {
                      const reqs = [...edit.f.reqs];
                      reqs[i] = { ...reqs[i], key: e.target.value };
                      setEdit({ ...edit, f: { ...edit.f, reqs } });
                    }}
                    className={`${inp} w-44`}
                  >
                    {METRIC_OPTIONS.map((m) => (
                      <option key={m.key} value={m.key}>
                        {m.label}（{m.unit}）
                      </option>
                    ))}
                  </select>
                  <input
                    type="number"
                    min="0"
                    value={r.value}
                    onChange={(e) => {
                      const reqs = [...edit.f.reqs];
                      reqs[i] = { ...reqs[i], value: e.target.value };
                      setEdit({ ...edit, f: { ...edit.f, reqs } });
                    }}
                    className={`${inp} w-24`}
                  />
                  <span className="text-xs text-sub">{opt?.unit ?? ""}</span>
                  {opt && <span className="text-xs text-sub">{opt.hint}</span>}
                  <button
                    type="button"
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={edit.f.reqs.length <= 1}
                    onClick={() =>
                      setEdit({
                        ...edit,
                        f: {
                          ...edit.f,
                          reqs: edit.f.reqs.filter((_, j) => j !== i),
                        },
                      })
                    }
                  >
                    删除
                  </button>
                </div>
              );
            })}
          </div>
          <button
            type="button"
            className={ADD_METRIC_BTN_CLS}
            onClick={() =>
              setEdit({
                ...edit,
                f: {
                  ...edit.f,
                  reqs: [...edit.f.reqs, { key: "uploads", value: "10" }],
                },
              })
            }
          >
            + 添加指标
          </button>
        </div>

        {/* 加成规则：小白化（每 N 个达标月 +M%） */}
        <div className="mt-3 flex flex-wrap items-end gap-2">
          <p className="w-full text-xs font-bold">连续达标加成</p>
          <span className="pb-2 text-xs text-sub">每</span>
          <label className="flex flex-col gap-1 text-xs">
            <span className="sr-only">档位月数</span>
            <input
              type="number"
              min="1"
              value={edit.f.bonusStep}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, bonusStep: e.target.value },
                })
              }
              className={`${inp} w-16`}
            />
          </label>
          <span className="pb-2 text-xs text-sub">个达标月，工资加</span>
          <label className="flex flex-col gap-1 text-xs">
            <span className="sr-only">加成百分比</span>
            <input
              type="number"
              min="0"
              max="100"
              value={edit.f.bonusPct}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, bonusPct: e.target.value } })
              }
              className={`${inp} w-16`}
            />
          </label>
          <span className="pb-2 text-xs text-sub">
            %（此岗位结算即按此比例；留空或 0 则用全站默认）
          </span>
        </div>

        <div className="mt-3 flex gap-2">
          <button
            className="baozi-button"
            disabled={busy || !edit.f.name.trim()}
            onClick={save}
          >
            保存
          </button>
          {edit.id !== null && (
            <button
              className={BTN_SM_BOLD}
              onClick={() =>
                setEdit({
                  id: null,
                  f: { ...EMPTY_FORM, reqs: [...EMPTY_FORM.reqs] },
                })
              }
            >
              取消
            </button>
          )}
        </div>
      </section>

      {/* 岗位表（拆至 ./admin-exams-table.tsx） */}
      <ExamsTable
        rows={rows}
        busy={busy}
        currency={currency}
        flash={flash}
        load={load}
        startEdit={startEdit}
      />
    </div>
  );
}
