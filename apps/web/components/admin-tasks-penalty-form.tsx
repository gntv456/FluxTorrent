"use client";

import { INPUT_CLOUD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import {
  PENALTY_KEYS,
  PENALTY_AMOUNT_KEYS,
  PENALTY_PAGES_KEYS,
  PENALTY_TARGET_KEYS,
} from "./admin-tasks-shared";

const inp = INPUT_CLOUD;

/** 0397 处置菜单编辑器（从 admin-tasks-form.tsx 拆出守 300 行门禁）。
 *  考核（kind≠task）未达标时可多选的惩罚组合；带参数的项
 *  （金额/目标量/页面列表）随勾选展开内联输入。 */

/** 单个参数字段的统一更新器（勾选项参数都在 penaltyParams[kind] 下） */
function setParam(
  edit: PenaltyFormState,
  setEdit: (v: PenaltyFormState) => void,
  kind: string,
  patch: Record<string, string>,
) {
  const prev = edit.f.penaltyParams[kind] ?? {};
  setEdit({
    ...edit,
    f: {
      ...edit.f,
      penaltyParams: {
        ...edit.f.penaltyParams,
        [kind]: { ...prev, ...patch },
      },
    },
  });
}

export interface PenaltyFormState {
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
    penaltyChecked: Record<string, boolean>;
    penaltyParams: Record<
      string,
      { amount?: string; target?: string; pages?: string }
    >;
  };
}

export function PenaltyMenuEdit(props: {
  edit: PenaltyFormState;
  setEdit:
    | React.Dispatch<React.SetStateAction<PenaltyFormState>>
    | ((v: PenaltyFormState) => void);
}) {
  const { edit, setEdit } = props;
  const { dict } = useI18n();
  const t = dict.adminTasks;
  return (
    <div className="mt-3">
      <p className="mb-1 text-xs font-bold">{t.penaltyTitle}</p>
      <p className="mb-2 text-xs text-sub">{t.penaltyHint}</p>
      <div className="flex flex-wrap gap-x-4 gap-y-2">
        {PENALTY_KEYS.map((k) => {
          const on = !!edit.f.penaltyChecked[k];
          const p = edit.f.penaltyParams[k] ?? {};
          return (
            <div key={k} className="flex flex-wrap items-center gap-1 text-xs">
              <label className="flex items-center gap-1">
                <input
                  type="checkbox"
                  checked={on}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      f: {
                        ...edit.f,
                        penaltyChecked: {
                          ...edit.f.penaltyChecked,
                          [k]: e.target.checked,
                        },
                      },
                    })
                  }
                />
                {t.penaltyKinds[k]}
              </label>
              {on && PENALTY_AMOUNT_KEYS.has(k) && (
                <input
                  type="number"
                  min="0"
                  value={p.amount ?? ""}
                  onChange={(e) =>
                    setParam(edit, setEdit, k, { amount: e.target.value })
                  }
                  placeholder={t.penaltyAmtPh}
                  className={`${inp} w-24`}
                />
              )}
              {on && PENALTY_TARGET_KEYS.has(k) && (
                <select
                  value={p.target ?? "uploaded"}
                  onChange={(e) =>
                    setParam(edit, setEdit, k, { target: e.target.value })
                  }
                  className={inp}
                >
                  <option value="uploaded">{t.penaltyTgtUploaded}</option>
                  <option value="downloaded">{t.penaltyTgtDownloaded}</option>
                </select>
              )}
              {on && PENALTY_PAGES_KEYS.has(k) && (
                <input
                  value={p.pages ?? ""}
                  onChange={(e) =>
                    setParam(edit, setEdit, k, { pages: e.target.value })
                  }
                  placeholder={t.penaltyPagesPh}
                  className={`${inp} w-48`}
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
