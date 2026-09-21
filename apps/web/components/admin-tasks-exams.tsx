"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 考核记录面板（从 admin-tasks.tsx 按域拆出，300 门禁）：
 *  /admin/exam-users 浏览（对标 NP exam-users）+ 0105 豁免/恢复。 */

interface ExamUserRow {
  claim_id: number;
  task_id: number;
  task_name: string;
  kind: string;
  period: string;
  user_id: number;
  username: string;
  status: number;
  claimed_at: string;
  settled_at: string | null;
  reward_paid: number | null;
  /** 0105 豁免标记：非空 = 暂不参与结算 */
  exempted_at: string | null;
}

const KINDS = [
  { v: "task", label: "普通任务" },
  { v: "onboard", label: "新人转正考核" },
  { v: "periodic", label: "周期考核" },
];

export function ExamUsersPanel({ flash }: { flash: (m: string) => void }) {
  const [exams, setExams] = useState<ExamUserRow[]>([]);
  const [examFilter, setExamFilter] = useState({
    user_id: "",
    task_id: "",
    status: "",
  });
  const [busy, setBusy] = useState(false);
  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

  const loadExams = useCallback(async () => {
    const q = new URLSearchParams();
    if (examFilter.user_id) q.set("user_id", examFilter.user_id);
    if (examFilter.task_id) q.set("task_id", examFilter.task_id);
    if (examFilter.status) q.set("status", examFilter.status);
    try {
      setExams(
        await api.get<ExamUserRow[]>(
          `/api/v1/admin/exam-users${q.size ? `?${q}` : ""}`,
        ),
      );
    } catch {
      /* 无考核记录 */
    }
  }, [examFilter]);

  useEffect(() => {
    loadExams();
  }, [loadExams]);

  /** 豁免 / 恢复（0105，对标 NP exam-users 的 avoid/recover）：
   *  豁免后该记录暂不参与结算，恢复后回到结算流。 */
  async function setExempt(claimId: number, exempt: boolean) {
    setBusy(true);
    try {
      await api.post(
        `/api/v1/admin/exam-users/${claimId}/${exempt ? "exempt" : "recover"}`,
      );
      flash(exempt ? "已豁免（该记录暂不参与结算）" : "已恢复");
      await loadExams();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">考核记录</h2>
      <div className="mb-2 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          UID
          <input
            type="number"
            value={examFilter.user_id}
            onChange={(e) =>
              setExamFilter({ ...examFilter, user_id: e.target.value })
            }
            className={`${inp} w-24`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          任务ID
          <input
            type="number"
            value={examFilter.task_id}
            onChange={(e) =>
              setExamFilter({ ...examFilter, task_id: e.target.value })
            }
            className={`${inp} w-24`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          状态
          <select
            value={examFilter.status}
            onChange={(e) =>
              setExamFilter({ ...examFilter, status: e.target.value })
            }
            className={inp}
          >
            <option value="">全部</option>
            <option value="0">进行中</option>
            <option value="1">已完成</option>
            <option value="2">已失败</option>
          </select>
        </label>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">用户</td>
              <td className="colhead">考核</td>
              <td className="colhead">类型</td>
              <td className="colhead">状态</td>
              <td className="colhead">派发/领取时间</td>
              <td className="colhead">结算时间</td>
              <td className="colhead">实发奖励</td>
              <td className="colhead">豁免</td>
              <td className="colhead text-right">操作</td>
            </tr>
          </thead>
          <tbody>
            {exams.map((e) => (
              <tr key={e.claim_id}>
                <td className="num">
                  {e.user_id} · {e.username}
                </td>
                <td className="font-bold">{e.task_name}</td>
                <td>{KINDS.find((k) => k.v === e.kind)?.label ?? e.kind}</td>
                <td>
                  {e.status === 0
                    ? "进行中"
                    : e.status === 1
                      ? "已完成"
                      : "已失败"}
                </td>
                <td className="text-sub">
                  {new Date(e.claimed_at).toLocaleString()}
                </td>
                <td className="text-sub">
                  {e.settled_at ? new Date(e.settled_at).toLocaleString() : "—"}
                </td>
                <td className="num">{e.reward_paid ?? "—"}</td>
                <td>
                  {e.exempted_at ? (
                    <span className="font-bold text-[var(--baozi-orange-dark)]">
                      已豁免
                    </span>
                  ) : (
                    "—"
                  )}
                </td>
                <td className="text-right">
                  {e.status === 0 &&
                    (e.exempted_at ? (
                      <button
                        className="cmgmt-act"
                        disabled={busy}
                        onClick={() => setExempt(e.claim_id, false)}
                      >
                        恢复
                      </button>
                    ) : (
                      <button
                        className="cmgmt-act cmgmt-act--danger"
                        disabled={busy}
                        onClick={() => setExempt(e.claim_id, true)}
                      >
                        豁免
                      </button>
                    ))}
                </td>
              </tr>
            ))}
            {exams.length === 0 && (
              <tr>
                <td colSpan={9} className="py-6 text-center text-sub">
                  暂无考核记录
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
