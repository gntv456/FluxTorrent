"use client";

import { useI18n } from "@/i18n/client";
import { BTN_SM_BOLD, INPUT_CLOUD } from "@/lib/ui-classes";
import { type ReqRow, EMPTY_FORM, metricOptions } from "./admin-exams-shared";

const inp = INPUT_CLOUD;
const ADD_METRIC_BTN_CLS = BTN_SM_BOLD;

/** 考核管理·新建/编辑表单（从 admin-exams.tsx 按域拆出）。 */
export function ExamsForm(props: {
  edit: { id: number | null; f: typeof EMPTY_FORM };
  setEdit: React.Dispatch<
    React.SetStateAction<{ id: number | null; f: typeof EMPTY_FORM }>
  >;
  save: () => void;
  busy: boolean;
}) {
  const { edit, setEdit, save, busy } = props;
  const { currency } = useI18n();
  const METRIC_OPTIONS = metricOptions(currency);
  return (
    <section className="baozi-panel cmgmt-form p-4">
      <h2 className="mb-2 text-base font-bold">
        {edit.id === null ? "新建考核岗位" : `编辑考核岗位 #${edit.id}`}
      </h2>
      <p className="mb-3 text-xs text-sub">
        登记入口在「绩效考核」面板批量分配，或用户详情页单人分配；这里维护岗位、指标门槛与加成。
      </p>

      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          岗位名
          <input
            value={edit.f.name}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
            }
            className={`${inp} w-36`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          底薪({currency})
          <input
            type="number"
            value={edit.f.base_pay}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, base_pay: e.target.value } })
            }
            className={`${inp} w-24`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          岗位说明
          <input
            value={edit.f.description}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, description: e.target.value },
              })
            }
            placeholder="展示给成员的岗位职责（可空）"
            className={`${inp} w-56`}
          />
        </label>
      </div>

      {/* 达标门槛：结构化行（指标下拉 + 数值），多行之间是 AND */}
      <div className="mt-3">
        <p className="mb-1 text-xs font-bold">达标门槛（全部满足才算达标）</p>
        <div className="flex flex-col gap-2">
          {edit.f.reqs.map((r, i) => {
            const opt = METRIC_OPTIONS.find((m) => m.key === r.key);
            return (
              <div key={i} className="flex flex-wrap items-center gap-2">
                <select
                  value={r.key}
                  onChange={(e) => {
                    const reqs = [...edit.f.reqs];
                    reqs[i] = { ...reqs[i], key: e.target.value };
                    setEdit({ ...edit, f: { ...edit.f, reqs } });
                  }}
                  className={`${inp} w-44`}
                >
                  {METRIC_OPTIONS.map((m) => (
                    <option key={m.key} value={m.key}>
                      {m.label}（{m.unit}）
                    </option>
                  ))}
                </select>
                <input
                  type="number"
                  min="0"
                  value={r.value}
                  onChange={(e) => {
                    const reqs = [...edit.f.reqs];
                    reqs[i] = { ...reqs[i], value: e.target.value };
                    setEdit({ ...edit, f: { ...edit.f, reqs } });
                  }}
                  className={`${inp} w-24`}
                />
                <span className="text-xs text-sub">{opt?.unit ?? ""}</span>
                {opt && <span className="text-xs text-sub">{opt.hint}</span>}
                <button
                  type="button"
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={edit.f.reqs.length <= 1}
                  onClick={() =>
                    setEdit({
                      ...edit,
                      f: {
                        ...edit.f,
                        reqs: edit.f.reqs.filter((_, j) => j !== i),
                      },
                    })
                  }
                >
                  删除
                </button>
              </div>
            );
          })}
        </div>
        <button
          type="button"
          className={ADD_METRIC_BTN_CLS}
          onClick={() =>
            setEdit({
              ...edit,
              f: {
                ...edit.f,
                reqs: [...edit.f.reqs, { key: "uploads", value: "10" }],
              },
            })
          }
        >
          + 添加指标
        </button>
      </div>

      {/* 加成规则：小白化（每 N 个达标月 +M%） */}
      <div className="mt-3 flex flex-wrap items-end gap-2">
        <p className="w-full text-xs font-bold">连续达标加成</p>
        <span className="pb-2 text-xs text-sub">每</span>
        <label className="flex flex-col gap-1 text-xs">
          <span className="sr-only">档位月数</span>
          <input
            type="number"
            min="1"
            value={edit.f.bonusStep}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, bonusStep: e.target.value },
              })
            }
            className={`${inp} w-16`}
          />
        </label>
        <span className="pb-2 text-xs text-sub">个达标月，工资加</span>
        <label className="flex flex-col gap-1 text-xs">
          <span className="sr-only">加成百分比</span>
          <input
            type="number"
            min="0"
            max="100"
            value={edit.f.bonusPct}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, bonusPct: e.target.value } })
            }
            className={`${inp} w-16`}
          />
        </label>
        <span className="pb-2 text-xs text-sub">
          %（此岗位结算即按此比例；留空或 0 则用全站默认）
        </span>
      </div>

      <div className="mt-3 flex gap-2">
        <button
          className="baozi-button"
          disabled={busy || !edit.f.name.trim()}
          onClick={save}
        >
          保存
        </button>
        {edit.id !== null && (
          <button
            className={BTN_SM_BOLD}
            onClick={() =>
              setEdit({
                id: null,
                f: { ...EMPTY_FORM, reqs: [...EMPTY_FORM.reqs] },
              })
            }
          >
            取消
          </button>
        )}
      </div>
    </section>
  );
}
