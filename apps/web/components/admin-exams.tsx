"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { ExamsForm } from "./admin-exams-form";

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
    <>
      <ExamsForm edit={edit} setEdit={setEdit} save={save} busy={busy} />
      <div className="flex flex-col gap-3">
        {msg && (
          <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
            {msg}
          </p>
        )}

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
    </>
  );
}
