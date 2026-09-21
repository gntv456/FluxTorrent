"use client";

/**
 * 任务定义·列表子面板（从 components/admin-tasks.tsx 按域拆出）：
 * 任务/考核定义表（类型、起止、目标等级、奖罚、期限、自动派发）
 * + 编辑回填与删除。数据加载留在父组件。
 */

import { api, ApiError } from "@/lib/api-client";
import type { TaskRow } from "./admin-tasks-shared";
import { KINDS, PERIODS, toLocalInput } from "./admin-tasks-shared";

interface TasksTableProps {
  rows: TaskRow[];
  busy: boolean;
  flash: (m: string) => void;
  load: () => Promise<void>;
  setEdit: (v: {
    id: number | null;
    f: {
      name: string;
      metric: string;
      starts_at: string;
      ends_at: string;
      target_class: string;
      reward: string;
      penalty: string;
      claim_limit: string;
      kind: string;
      auto_assign: boolean;
      period: string;
      duration_days: string;
      subtitle: string;
      tier: string;
      fee: string;
      quota_total: string;
      sort: string;
    };
  }) => void;
}

export function TasksTable({
  rows,
  busy,
  flash,
  load,
  setEdit,
}: TasksTableProps) {
  return (
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
              {t.kind !== "task" && (
                <span>
                  {" "}
                  · {PERIODS.find((p) => p.v === t.period)?.label ?? t.period}
                </span>
              )}
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
  );
}
