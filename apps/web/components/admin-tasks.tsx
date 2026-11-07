"use client";

import { BTN_SM_BOLD, INPUT_CLOUD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ExamUsersPanel } from "@/components/admin-tasks-exams";
import { TasksTable } from "./admin-tasks-table";
import { TasksForm } from "./admin-tasks-form";
import type { TaskRow } from "./admin-tasks-shared";
import {
  EMPTY,
  fromMetricJson,
  fromPenaltyItems,
  toMetricJson,
  toPenaltyItems,
} from "./admin-tasks-shared";

/** 任务定义 + 考核配置（tasks 表 CRUD，0093 起 kind/auto_assign/period 支撑考核引擎）。
 *  考核记录浏览拆出 admin-tasks-exams.tsx；列表拆至 ./admin-tasks-table.tsx；
 *  类型与常量拆至 ./admin-tasks-shared.ts（300 门禁）。 */

/** 圆角描边小按钮（取消编辑） */
const PLAIN_BTN_CLS = BTN_SM_BOLD;

export function AdminTasks() {
  const { dict } = useI18n();
  const t = dict.adminTasks;
  const [rows, setRows] = useState<TaskRow[]>([]);
  const [edit, setEdit] = useState<{ id: number | null; f: typeof EMPTY }>({
    id: null,
    f: { ...EMPTY },
  });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<TaskRow[]>("/api/v1/admin/tasks"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : t.loadFail);
    }
  }, [t.loadFail]);

  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    // 0472 指标可视化：勾选+数值由转换器换算（GB→字节/小时→秒），
    // 六键外的历史手写键经 extra 原样保留
    const metric = toMetricJson(
      edit.f.metricChecked,
      edit.f.metricValues,
      edit.f.metricExtra,
    );
    if (Object.keys(metric).length === 0) {
      flash(t.metricEmpty);
      return;
    }
    if (!edit.f.starts_at || !edit.f.ends_at) {
      flash(t.rangeRequired);
      return;
    }
    setBusy(true);
    try {
      const payload = {
        name: edit.f.name,
        metric,
        starts_at: new Date(edit.f.starts_at).toISOString(),
        ends_at: new Date(edit.f.ends_at).toISOString(),
        target_class: Number(edit.f.target_class) || 0,
        reward: Number(edit.f.reward) || 0,
        penalty: Number(edit.f.penalty) || 0,
        claim_limit: edit.f.claim_limit ? Number(edit.f.claim_limit) : null,
        kind: edit.f.kind,
        auto_assign: edit.f.kind !== "task" ? edit.f.auto_assign : false,
        period: edit.f.period,
        // 考核内容/方式（0093）：期限、副标题、口径、费用、配额、排序
        duration_days: Number(edit.f.duration_days) || 30,
        subtitle: edit.f.subtitle.trim() || null,
        tier: edit.f.tier.trim() || null,
        fee: Number(edit.f.fee) || 0,
        quota_total: Number(edit.f.quota_total) || 200,
        sort: Number(edit.f.sort) || 0,
        // 0397 处置菜单：仅考核类型提交（后端校验 kind 白名单）
        fail_penalty:
          edit.f.kind !== "task"
            ? toPenaltyItems(edit.f.penaltyChecked, edit.f.penaltyParams)
            : [],
      };
      if (edit.id === null) await api.post("/api/v1/admin/tasks", payload);
      else await api.put(`/api/v1/admin/tasks/${edit.id}`, payload);
      flash(t.saved);
      setEdit({ id: null, f: { ...EMPTY } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : t.actionFail);
    } finally {
      setBusy(false);
    }
  }

  const isExam = edit.f.kind !== "task";

  return (
    <>
      <TasksForm edit={edit} setEdit={setEdit} save={save} busy={busy} />
      <div className="flex flex-col gap-3">
        {msg && (
          <p
            className="rounded-[var(--r-md)] bg-[var(--sky-soft)]
                       p-3 text-sm text-[var(--ink)]"
          >
            {msg}
          </p>
        )}
        {/* 任务定义表（拆至 ./admin-tasks-table.tsx） */}
        <TasksTable
          rows={rows}
          busy={busy}
          flash={flash}
          load={load}
          setEdit={setEdit}
        />

        <ExamUsersPanel flash={flash} />
      </div>
    </>
  );
}
