"use client";

import { BTN_SM_BOLD as PLAIN_BTN_CLS, INPUT_CLOUD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { EMPTY, kindList, periodList } from "./admin-tasks-shared";

const inp = INPUT_CLOUD;

/** 任务新建/编辑表单（从 admin-tasks.tsx 按域拆出）。 */
export function TasksForm(props: {
  edit: { id: number | null; f: typeof EMPTY };
  setEdit: React.Dispatch<
    React.SetStateAction<{ id: number | null; f: typeof EMPTY }>
  >;
  save: () => void;
  busy: boolean;
}) {
  const { edit, setEdit, save, busy } = props;
  const { dict } = useI18n();
  const t = dict.adminTasks;
  const isExam = edit.f.kind !== "task";
  return (
    <section className="baozi-panel cmgmt-form p-4">
      <h2 className="mb-2 text-base font-bold">
        {edit.id === null
          ? t.formTitleNew
          : fmt(t.formTitleEdit, { id: edit.id })}
      </h2>
      <p className="mb-2 text-xs text-sub">{t.metricHint}</p>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {t.fldKind}
          <select
            value={edit.f.kind}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, kind: e.target.value } })
            }
            className={inp}
          >
            {kindList(t.kinds).map((k) => (
              <option key={k.v} value={k.v}>
                {k.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fldName}
          <input
            value={edit.f.name}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
            }
            className={`${inp} w-36`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fldMetric}
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
          {t.fldStart}
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
          {t.fldEnd}
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
          {t.fldTargetClass}
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
          {t.fldReward}
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
          {t.fldPenalty}
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
          {t.fldClaimLimit}
          <input
            type="number"
            value={edit.f.claim_limit}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, claim_limit: e.target.value },
              })
            }
            placeholder={t.claimLimitPh}
            className={`${inp} w-20`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fldSubtitle}
          <input
            value={edit.f.subtitle}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, subtitle: e.target.value } })
            }
            placeholder={t.subtitlePh}
            className={`${inp} w-56`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fldFee}
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
          {t.fldQuota}
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
          {t.fldSort}
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
              {t.fldPeriod}
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
                {periodList(t.periods).map((p) => (
                  <option key={p.v} value={p.v}>
                    {p.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {t.fldDuration}
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
              {t.fldTier}
              <input
                value={edit.f.tier}
                onChange={(e) =>
                  setEdit({ ...edit, f: { ...edit.f, tier: e.target.value } })
                }
                placeholder={t.tierPh}
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
              {t.fldAutoAssign}
            </label>
          </>
        )}
        <button
          className="baozi-button"
          disabled={busy || !edit.f.name.trim()}
          onClick={save}
        >
          {t.saveBtn}
        </button>
        {edit.id !== null && (
          <button
            className={PLAIN_BTN_CLS}
            onClick={() => setEdit({ id: null, f: { ...EMPTY } })}
          >
            {t.cancelBtn}
          </button>
        )}
      </div>
    </section>
  );
}
