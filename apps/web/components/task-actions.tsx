"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import type { TaskItem } from "@/lib/data";

/** 任务系统（客户端叶子）：列表 + 认领（后端唯一约束防重复） */
export function TaskList({ empty }: { empty: string }) {
  const [tasks, setTasks] = useState<TaskItem[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);

  async function refresh() {
    try {
      setTasks(await api.get<TaskItem[]>("/api/v1/tasks"));
    } catch {
      setTasks([]);
    }
  }
  useEffect(() => {
    refresh();
  }, []);

  async function claim(id: number) {
    setBusyId(id);
    setMsg(null);
    try {
      await api.post("/api/v1/tasks/claim", { task_id: id });
      setMsg("✓");
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusyId(null);
    }
  }

  if (tasks === null) return null;
  if (tasks.length === 0) return <p className="py-8 text-center text-sub">{empty}</p>;
  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p role="alert" className="text-sm text-sky-deep">
          {msg}
        </p>
      )}
      {tasks.map((t) => {
        const full = t.claim_limit !== null && t.claimed >= t.claim_limit;
        return (
          <div
            key={t.id}
            className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h2 className="font-bold">{t.name}</h2>
              <span className="sticker bg-sun text-ink num">+{t.reward}</span>
            </div>
            <p className="mt-1 text-xs text-sub">
              {t.claim_limit !== null
                ? `名额 ${t.claimed}/${t.claim_limit}`
                : `已认领 ${t.claimed}`}
              {t.penalty > 0 && ` · 未达标扣 ${t.penalty}`}
            </p>
            <button
              type="button"
              disabled={busyId === t.id || full}
              onClick={() => claim(t.id)}
              className="mt-2 min-h-[36px] rounded-full bg-sky-deep px-4 text-sm text-white disabled:opacity-50"
            >
              {full ? "名额已满" : "认领任务"}
            </button>
          </div>
        );
      })}
    </div>
  );
}
