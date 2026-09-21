"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { ExamUsersPanel } from "@/components/admin-tasks-exams";

/** 任务定义 + 考核配置（tasks 表 CRUD，0093 起 kind/auto_assign/period 支撑考核引擎）。
 *  考核记录浏览拆出 admin-tasks-exams.tsx（300 门禁）。 */

interface TaskRow {
  id: number;
  name: string;
  metric: Record<string, unknown>;
  starts_at: string;
  ends_at: string;
  target_class: number;
  reward: number;
  penalty: number;
  claim_limit: number | null;
  kind: string;
  auto_assign: boolean;
  period: string;
  /** 以下为 0093 考核引擎字段（本次补齐为可配） */
  duration_days: number;
  subtitle: string | null;
  tier: string | null;
  fee: number;
  quota_total: number;
  sort: number;
}

const EMPTY = {
  name: "",
  metric: "{}",
  starts_at: "",
  ends_at: "",
  target_class: "0",
  reward: "0",
  penalty: "0",
  claim_limit: "",
  kind: "task",
  auto_assign: false,
  period: "once",
  duration_days: "30",
  subtitle: "",
  tier: "",
  fee: "0",
  quota_total: "200",
  sort: "0",
};

const KINDS = [
  { v: "task", label: "普通任务" },
  { v: "onboard", label: "新人转正考核" },
  { v: "periodic", label: "周期考核" },
];
const PERIODS = [
  { v: "once", label: "一次性" },
  { v: "monthly", label: "每月" },
  { v: "quarterly", label: "每季" },
];

function toLocalInput(iso: string): string {
  const d = new Date(iso);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000)
    .toISOString()
    .slice(0, 16);
}

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
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";
  const isExam = edit.f.kind !== "task";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {edit.id === null ? "新建任务/考核" : `编辑 #${edit.id}`}
        </h2>
        <p className="mb-2 text-xs text-sub">
          可选指标键：upload_delta（上传增量，字节）·
          download_delta（累计口径，字节）·
          seed_seconds_delta（做种时长增量，秒，如 120h=432000）·
          seed_points_delta（做种积分，1 积分=1 小时做种）· uploads（发布数）·
          subtitles（字幕数）。至少配一个键，否则不可领取；tier
          任务按累计口径判定。
        </p>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">
            类型
            <select
              value={edit.f.kind}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, kind: e.target.value } })
              }
              className={inp}
            >
              {KINDS.map((k) => (
                <option key={k.v} value={k.v}>
                  {k.label}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">
            任务名
            <input
              value={edit.f.name}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
              }
              className={`${inp} w-36`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            指标 metric
            <input
              value={edit.f.metric}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, metric: e.target.value } })
              }
              placeholder='{"seed_seconds_delta":432000}'
              className={`${inp} w-64 font-mono`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            开始
            <input
              type="datetime-local"
              value={edit.f.starts_at}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, starts_at: e.target.value },
                })
              }
              className={inp}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            结束
            <input
              type="datetime-local"
              value={edit.f.ends_at}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, ends_at: e.target.value } })
              }
              className={inp}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            目标等级
            <input
              type="number"
              value={edit.f.target_class}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, target_class: e.target.value },
                })
              }
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            奖励
            <input
              type="number"
              value={edit.f.reward}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, reward: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            罚则
            <input
              type="number"
              value={edit.f.penalty}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, penalty: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            限领次数
            <input
              type="number"
              value={edit.f.claim_limit}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, claim_limit: e.target.value },
                })
              }
              placeholder="空=不限"
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            副标题
            <input
              value={edit.f.subtitle}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, subtitle: e.target.value } })
              }
              placeholder="展示在任务名下方"
              className={`${inp} w-56`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            费用
            <input
              type="number"
              value={edit.f.fee}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, fee: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            配额
            <input
              type="number"
              value={edit.f.quota_total}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, quota_total: e.target.value },
                })
              }
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            排序
            <input
              type="number"
              value={edit.f.sort}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, sort: e.target.value } })
              }
              className={`${inp} w-20`}
            />
          </label>
          {isExam && (
            <>
              <label className="flex flex-col gap-1 text-xs">
                周期
                <select
                  value={edit.f.period}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      f: { ...edit.f, period: e.target.value },
                    })
                  }
                  className={inp}
                >
                  {PERIODS.map((p) => (
                    <option key={p.v} value={p.v}>
                      {p.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-col gap-1 text-xs">
                考核期限（天）
                <input
                  type="number"
                  value={edit.f.duration_days}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      f: { ...edit.f, duration_days: e.target.value },
                    })
                  }
                  className={`${inp} w-24`}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                累计口径 tier
                <input
                  value={edit.f.tier}
                  onChange={(e) =>
                    setEdit({ ...edit, f: { ...edit.f, tier: e.target.value } })
                  }
                  placeholder="空=增量口径"
                  className={`${inp} w-28`}
                />
              </label>
              <label className="flex items-center gap-1 pb-2 text-xs">
                <input
                  type="checkbox"
                  checked={edit.f.auto_assign}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      f: { ...edit.f, auto_assign: e.target.checked },
                    })
                  }
                />
                自动派发（onboard=注册 N 天内新人；periodic=达标等级全体）
              </label>
            </>
          )}
          <button
            className="baozi-button"
            disabled={busy || !edit.f.name.trim()}
            onClick={save}
          >
            保存
          </button>
          {edit.id !== null && (
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() => setEdit({ id: null, f: { ...EMPTY } })}
            >
              取消
            </button>
          )}
        </div>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">任务</td>
            <td className="colhead">类型</td>
            <td className="colhead">起止</td>
            <td className="colhead">目标等级</td>
            <td className="colhead">奖励/罚则</td>
            <td className="colhead">限领</td>
            <td className="colhead">期限</td>
            <td className="colhead">自动派发</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((t) => (
            <tr key={t.id}>
              <td className="num">{t.id}</td>
              <td className="font-bold">{t.name}</td>
              <td>
                {KINDS.find((k) => k.v === t.kind)?.label ?? t.kind}
                {t.kind !== "task" &&
                  ` · ${PERIODS.find((p) => p.v === t.period)?.label ?? t.period}`}
              </td>
              <td className="text-sub">
                {new Date(t.starts_at).toLocaleDateString()} ~{" "}
                {new Date(t.ends_at).toLocaleDateString()}
              </td>
              <td className="num">{t.target_class}</td>
              <td className="num">
                {t.reward} / {t.penalty}
              </td>
              <td className="num">{t.claim_limit ?? "—"}</td>
              <td className="num">
                {t.kind === "task" ? "—" : `${t.duration_days} 天`}
              </td>
              <td>{t.kind === "task" ? "—" : t.auto_assign ? "是" : "否"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEdit({
                      id: t.id,
                      f: {
                        name: t.name,
                        metric: JSON.stringify(t.metric),
                        starts_at: toLocalInput(t.starts_at),
                        ends_at: toLocalInput(t.ends_at),
                        target_class: String(t.target_class),
                        reward: String(t.reward),
                        penalty: String(t.penalty),
                        claim_limit: t.claim_limit ? String(t.claim_limit) : "",
                        kind: t.kind ?? "task",
                        auto_assign: !!t.auto_assign,
                        period: t.period ?? "once",
                        duration_days: String(t.duration_days ?? 30),
                        subtitle: t.subtitle ?? "",
                        tier: t.tier ?? "",
                        fee: String(t.fee ?? 0),
                        quota_total: String(t.quota_total ?? 200),
                        sort: String(t.sort ?? 0),
                      },
                    })
                  }
                >
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/tasks/${t.id}`);
                      flash("已删除");
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "删除失败");
                    }
                  }}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={10} className="py-6 text-center text-sub">
                暂无任务
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <ExamUsersPanel flash={flash} />
    </div>
  );
}
