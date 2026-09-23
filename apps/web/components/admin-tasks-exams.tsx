"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { kindList } from "./admin-tasks-shared";

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

export function ExamUsersPanel({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const t = dict.adminTasks;
  const KINDS = kindList(dict.adminTasks.kinds);
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
      flash(exempt ? t.exemptedMsg : t.recoveredMsg);
      await loadExams();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : t.actionFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">{t.examsTitle}</h2>
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
          {t.fldTaskId}
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
          {t.fldStatus}
          <select
            value={examFilter.status}
            onChange={(e) =>
              setExamFilter({ ...examFilter, status: e.target.value })
            }
            className={inp}
          >
            <option value="">{t.stAll}</option>
            <option value="0">{t.stOngoing}</option>
            <option value="1">{t.stDone}</option>
            <option value="2">{t.stFailed}</option>
          </select>
        </label>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{t.colUser}</td>
              <td className="colhead">{t.colExam}</td>
              <td className="colhead">{t.colKind}</td>
              <td className="colhead">{t.colStatus}</td>
              <td className="colhead">{t.colAssignTime}</td>
              <td className="colhead">{t.colSettleTime}</td>
              <td className="colhead">{t.colRewardPaid}</td>
              <td className="colhead">{t.colExempt}</td>
              <td className="colhead text-right">{t.colActions}</td>
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
                    ? t.stOngoing
                    : e.status === 1
                      ? t.stDone
                      : t.stFailed}
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
                      {t.exempted}
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
                        {t.recoverBtn}
                      </button>
                    ) : (
                      <button
                        className="cmgmt-act cmgmt-act--danger"
                        disabled={busy}
                        onClick={() => setExempt(e.claim_id, true)}
                      >
                        {t.exemptBtn}
                      </button>
                    ))}
                </td>
              </tr>
            ))}
            {exams.length === 0 && (
              <tr>
                <td colSpan={9} className="py-6 text-center text-sub">
                  {t.examsEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
