"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { ExamUsersPanel } from "@/components/admin-tasks-exams";
import { TasksTable } from "./admin-tasks-table";
import { TasksForm } from "./admin-tasks-form";
import type { TaskRow } from "./admin-tasks-shared";
import { EMPTY, KINDS, PERIODS } from "./admin-tasks-shared";

/** 任务定义 + 考核配置（tasks 表 CRUD，0093 起 kind/auto_assign/period 支撑考核引擎）。
 *  考核记录浏览拆出 admin-tasks-exams.tsx；列表拆至 ./admin-tasks-table.tsx；
 *  类型与常量拆至 ./admin-tasks-shared.ts（300 门禁）。 */

/** 圆角描边小按钮（取消编辑） */
const PLAIN_BTN_CLS = BTN_SM_BOLD;

export function AdminTasks() {
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
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    let metric: unknown;
    try {
      metric = JSON.parse(edit.f.metric || "{}");
    } catch {
      flash("metric 需为合法 JSON");
      return;
    }
    if (!edit.f.starts_at || !edit.f.ends_at) {
      flash("起止时间必填");
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
      };
      if (edit.id === null) await api.post("/api/v1/admin/tasks", payload);
      else await api.put(`/api/v1/admin/tasks/${edit.id}`, payload);
      flash("已保存");
      setEdit({ id: null, f: { ...EMPTY } });
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
  const isExam = edit.f.kind !== "task";

  return (
    <>
      <TasksForm edit={edit} setEdit={setEdit} save={save} busy={busy} />
      <div className="flex flex-col gap-3">
        {msg && (
          <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
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
