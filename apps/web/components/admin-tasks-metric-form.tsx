"use client";

import { INPUT_CLOUD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { METRIC_ITEMS } from "./admin-tasks-shared";

const inp = INPUT_CLOUD;

/** 0472 指标可视化编辑器（自 admin-tasks-form.tsx 拆出守 300 门禁；
 *  模式同 PenaltyMenuEdit：勾选指标 + 按单位填数值，提交时换算——
 *  上传/下载 GB→字节、做种时长 小时→秒，计数直填）。 */
export function MetricMenuEdit(props: {
  edit: { id: number | null; f: typeof import("./admin-tasks-shared").EMPTY };
  setEdit: React.Dispatch<
    React.SetStateAction<{
      id: number | null;
      f: typeof import("./admin-tasks-shared").EMPTY;
    }>
  >;
}) {
  const { edit, setEdit } = props;
  const { dict } = useI18n();
  const t = dict.adminTasks;
  const mLabel = (key: string) =>
    (t.metricNames as Record<string, string> | undefined)?.[key] ?? key;
  return (
    <div
      className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3"
      data-testid="metric-grid"
    >
      {METRIC_ITEMS.map((m) => {
        const on = !!edit.f.metricChecked[m.key];
        const unitLabel =
          m.unit === "gb"
            ? t.metricUnitGB
            : m.unit === "hours"
              ? t.metricUnitHours
              : t.metricUnitCount;
        const cell =
          "flex min-h-[40px] items-center gap-2 rounded-[var(--r-sm)] " +
          "border px-2 py-1.5 text-xs " +
          (on
            ? "border-sky bg-[var(--surface-card)]"
            : "border-line bg-transparent");
        return (
          <label key={m.key} className={cell}>
            <input
              type="checkbox"
              checked={on}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: {
                    ...edit.f,
                    metricChecked: {
                      ...edit.f.metricChecked,
                      [m.key]: e.target.checked,
                    },
                  },
                })
              }
            />
            <span className="w-20 shrink-0">{mLabel(m.key)}</span>
            {on && (
              <>
                <input
                  type="number"
                  min="0"
                  step={m.step}
                  value={edit.f.metricValues[m.key] ?? ""}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      f: {
                        ...edit.f,
                        metricValues: {
                          ...edit.f.metricValues,
                          [m.key]: e.target.value,
                        },
                      },
                    })
                  }
                  className={`${inp} w-20 min-w-0 flex-1`}
                  placeholder="0"
                />
                <span className="shrink-0 text-sub">{unitLabel}</span>
              </>
            )}
          </label>
        );
      })}
    </div>
  );
}
