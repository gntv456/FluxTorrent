"use client";

import { BTN_SM_BOLD as PLAIN_BTN_CLS, INPUT_CLOUD } from "@/lib/ui-classes";
import { EMPTY, KINDS, PERIODS } from "./admin-tasks-shared";

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
  const isExam = edit.f.kind !== "task";
  return (
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
            className={PLAIN_BTN_CLS}
            onClick={() => setEdit({ id: null, f: { ...EMPTY } })}
          >
            取消
          </button>
        )}
      </div>
    </section>
  );
}
