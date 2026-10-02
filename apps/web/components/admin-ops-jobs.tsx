"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 任务调度面板（0218 G7）：全量 job 目录 / 节奏 / 最近运行 + 手动触发入队。
 *  「执行」只写 job_triggers，worker 在自己 tick 里用**本体函数**跑
 *  （同一把 advisory 锁，与定时触发互斥），结果回写 job_status/job_triggers。
 *  此前这里是「与 worker 同款 SQL 的复刻」（/run2），job 演进后会静默漂移，
 *  且只覆盖 10/40+ 个任务。 */

interface JobItem {
  job: string;
  cadence: string;
  last_started_at: string | null;
  last_finished_at: string | null;
  last_ok: boolean | null;
  last_result: string | null;
  /** 0225 G30-B：最近一次执行的实例标识（多 worker 分实例可见） */
  executed_by: string;
}

interface TriggerItem {
  id: number;
  job: string;
  status: string;
  requested_at: string;
  finished_at: string | null;
  ok: boolean | null;
  result: string | null;
}

interface JobsPayload {
  jobs: JobItem[];
  triggers: TriggerItem[];
}

/** 节奏展示顺序（后端只按字符串排序，粒度顺序要前端定） */
const CADENCE_ORDER = ["60s", "10m", "30m", "1h", "6h", "24h"];

function cadenceRank(c: string): number {
  const i = CADENCE_ORDER.indexOf(c);
  return i < 0 ? CADENCE_ORDER.length : i;
}

const BTN_SM =
  "min-h-[32px] rounded-full border border-line px-3 text-xs " +
  "font-bold disabled:opacity-50";

export function AdminOpsJobs() {
  const { dict, locale } = useI18n();
  const t = dict.adminops;
  const [data, setData] = useState<JobsPayload | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setData(await api.get<JobsPayload>("/api/v1/admin/jobs"));
    } catch {
      /* 面板级静默：保留上一次数据，下轮轮询再试 */
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  // 有待执行/执行中时每 3s 刷新，队列清空即停（不空转）
  const active = (data?.triggers ?? []).some(
    (x) => x.status === "pending" || x.status === "running",
  );
  useEffect(() => {
    if (!active) return;
    const id = setInterval(load, 3000);
    return () => clearInterval(id);
  }, [active, load]);

  async function run(job: string) {
    setBusy(job);
    setMsg(null);
    try {
      const r = await api.post<{ id: number; coalesced: boolean }>(
        "/api/v1/admin/jobs/run",
        { job },
      );
      setMsg(fmt(r.coalesced ? t.jobsCoalesced : t.jobsQueued, { job }));
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.runFailed);
    } finally {
      setBusy(null);
    }
  }

  const cadenceLabel = (c: string) =>
    t.cadences.find(([k]) => k === c)?.[1] ?? c;
  const timeOf = (s: string | null) =>
    s ? new Date(s).toLocaleString(dateLocale(locale)) : "—";
  const statusOf = (s: string, ok: boolean | null) =>
    s === "pending"
      ? t.jobsPending
      : s === "running"
        ? t.jobsRunning
        : ok
          ? t.jobsOk
          : t.jobsFail;

  const jobs = data?.jobs ?? [];
  const triggers = data?.triggers ?? [];
  const byCadence = new Map<string, JobItem[]>();
  for (const j of jobs) {
    const arr = byCadence.get(j.cadence);
    if (arr) arr.push(j);
    else byCadence.set(j.cadence, [j]);
  }
  const groups = [...byCadence.entries()].sort(
    (a, b) =>
      cadenceRank(a[0]) - cadenceRank(b[0]) || a[0].localeCompare(b[0]),
  );

  return (
    <div className="baozi-panel flex flex-col gap-2 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-base font-bold">⚙️ {t.jobsTitle}</h2>
        {active && (
          <span className="text-xs text-sub">{t.jobsPolling}</span>
        )}
      </div>
      <p className="text-xs text-sub">{t.jobsNote}</p>
      {msg && (
        <p
          className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink"
          role="status"
        >
          {msg}
        </p>
      )}
      {jobs.length === 0 && (
        <p className="py-4 text-center text-xs text-sub">{t.jobsEmpty}</p>
      )}

      {groups.map(([cad, items]) => (
        <section key={cad} className="flex flex-col gap-1">
          <h3 className="text-xs font-bold text-sub">
            {fmt(t.jobsGroup, {
              cadence: cadenceLabel(cad),
              n: items.length,
            })}
          </h3>
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table text-xs">
              <tbody>
                <tr>
                  <td className="colhead">{t.jobsJob}</td>
                  <td className="colhead w-44">{t.jobsLast}</td>
                  <td className="colhead w-16">{t.jobsState}</td>
                  <td className="colhead w-28">{t.jobsBy}</td>
                  <td className="colhead">{t.jobsResult}</td>
                  <td className="colhead w-20" />
                </tr>
                {items.map((j) => (
                  <tr key={j.job}>
                    <td className="font-mono">{j.job}</td>
                    <td className="text-sub">
                      {j.last_finished_at
                        ? timeOf(j.last_finished_at)
                        : t.jobsNever}
                    </td>
                    <td
                      className={
                        j.last_ok === null
                          ? "text-sub"
                          : j.last_ok
                            ? "text-success"
                            : "text-danger"
                      }
                    >
                      {j.last_ok === null
                        ? // NULL = 从未执行（多为所属模块关闭，worker 注册目录但不跑）
                          // ——与「执行过但失败」区分，避免面板满屏红叉误导
                          (j.last_started_at === null
                            ? t.jobsIdle
                            : "—")
                        : j.last_ok
                          ? t.jobsOk
                          : t.jobsFail}
                    </td>
                    <td className="font-mono text-sub">
                      {j.executed_by || "—"}
                    </td>
                    <td className="break-all text-sub">
                      {j.last_result ?? "—"}
                    </td>
                    <td className="text-right">
                      <button
                        type="button"
                        disabled={busy !== null}
                        onClick={() => run(j.job)}
                        className={BTN_SM}
                      >
                        {busy === j.job ? "…" : t.jobRun}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      ))}

      {triggers.length > 0 && (
        <section className="flex flex-col gap-1">
          <h3 className="text-xs font-bold text-sub">{t.jobsTriggers}</h3>
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table text-xs">
              <tbody>
                <tr>
                  <td className="colhead w-10">#</td>
                  <td className="colhead">{t.jobsJob}</td>
                  <td className="colhead w-16">{t.jobsState}</td>
                  <td className="colhead w-44">{t.jobsRequested}</td>
                  <td className="colhead w-44">{t.jobsFinished}</td>
                  <td className="colhead">{t.jobsResult}</td>
                </tr>
                {triggers.map((r) => (
                  <tr key={r.id}>
                    <td className="num">{r.id}</td>
                    <td className="font-mono">{r.job}</td>
                    <td
                      className={
                        r.status === "failed"
                          ? "text-danger"
                          : r.status === "done"
                            ? "text-success"
                            : "text-sub"
                      }
                    >
                      {statusOf(r.status, r.ok)}
                    </td>
                    <td className="text-sub">{timeOf(r.requested_at)}</td>
                    <td className="text-sub">{timeOf(r.finished_at)}</td>
                    <td className="break-all text-sub">
                      {r.result ?? "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}
