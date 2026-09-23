"use client";

import { EMPTY_EDIT } from "./admin-props";

import { useI18n } from "@/i18n/client";
import { BTN_SM_BOLD as PLAIN_BTN_CLS, INPUT_CLOUD } from "@/lib/ui-classes";
import type { FrameRow } from "./admin-props-shared";
import {
  cfgField,
  setCfgField,
  kindFields,
  kindLabelMap,
  type EditState,
} from "./admin-props-shared";

const inp = INPUT_CLOUD;
const WIDE_CLS = "flex-1";

/** 道具管理·新建/编辑表单（从 admin-props.tsx 按域拆出）。 */
export function PropsForm(props: {
  edit: EditState;
  setEdit: React.Dispatch<React.SetStateAction<EditState>>;
  save: () => void;
  busy: boolean;
  frames: FrameRow[];
}) {
  const { edit, setEdit, save, busy, frames } = props;
  const { currency, dict } = useI18n();
  const KIND_LABEL = kindLabelMap(dict.adminProps.kindLabel);
  const KIND_FIELDS = kindFields(dict.adminProps.fields, dict.adminProps.opts);
  return (
    <section className="baozi-panel cmgmt-form p-4">
      <h2 className="mb-2 text-base font-bold">
        {edit.id === null ? "新建道具" : `编辑道具 #${edit.id}`}
      </h2>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          名称
          <input
            value={edit.f.name}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
            }
            className={`${inp} w-36`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          类型
          <select
            value={edit.f.kind}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, kind: e.target.value } })
            }
            className={inp}
          >
            {Object.entries(KIND_LABEL).map(([k, l]) => (
              <option key={k} value={k}>
                {l.replaceAll("CURRENCY", currency)}（{k}）
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          价格({currency})
          <input
            type="number"
            value={edit.f.price}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, price: e.target.value } })
            }
            className={`${inp} w-24`}
          />
        </label>

        {/* —— 效果配置：按类型出结构化字段，自动拼 config（参考 NP Filament 后台的字段式表单） —— */}
        {KIND_FIELDS[edit.f.kind]?.map((fd) => (
          <label
            key={fd.key}
            className={`flex flex-col gap-1 text-xs ${fd.wide ? WIDE_CLS : ""}`.trim()}
          >
            {fd.label.replace("CURRENCY", currency)}
            {fd.options ? (
              <select
                value={cfgField(edit.f.config)[fd.key] ?? ""}
                onChange={(e) =>
                  setEdit({
                    ...edit,
                    f: {
                      ...edit.f,
                      config: setCfgField(
                        edit.f.config,
                        fd.key,
                        e.target.value,
                      ),
                    },
                  })
                }
                className={`${inp} ${fd.wide ? "min-w-[140px]" : "w-40"}`}
              >
                <option value="">（不选）</option>
                {fd.options === "frames"
                  ? frames.map((fr) => (
                      <option key={fr.id} value={String(fr.id)}>
                        {fr.name}（#{fr.id}）
                      </option>
                    ))
                  : fd.options.map(([v, l]) => (
                      <option key={v} value={v}>
                        {l}
                      </option>
                    ))}
              </select>
            ) : (
              <input
                type={fd.num ? "number" : "text"}
                value={cfgField(edit.f.config)[fd.key] ?? ""}
                placeholder={fd.ph ?? ""}
                onChange={(e) =>
                  setEdit({
                    ...edit,
                    f: {
                      ...edit.f,
                      config: setCfgField(
                        edit.f.config,
                        fd.key,
                        e.target.value,
                      ),
                    },
                  })
                }
                className={[
                  inp,
                  fd.wide ? "min-w-[200px]" : "w-36",
                  fd.num ? "" : "font-mono",
                ]
                  .join(" ")
                  .trim()}
              />
            )}
          </label>
        ))}

        <label className="flex flex-col gap-1 text-xs">
          config(JSON，专家模式可直接改)
          <input
            value={edit.f.config}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, config: e.target.value } })
            }
            className={`${inp} w-52 font-mono`}
          />
        </label>
        <label className="flex items-center gap-1 pb-2 text-xs">
          <input
            type="checkbox"
            checked={edit.f.active}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, active: e.target.checked },
              })
            }
          />
          上架
        </label>
        <button
          className="baozi-button"
          disabled={busy || !edit.f.name.trim()}
          onClick={save}
        >
          保存
        </button>
        {edit.id !== null && (
          <button className={PLAIN_BTN_CLS} onClick={() => setEdit(EMPTY_EDIT)}>
            取消
          </button>
        )}
      </div>
      <p className="mt-2 text-xs text-sub">
        即时生效类（上传量/{currency}
        /邀请）发放直接入账；卡牌/装饰类入背包待用户使用。装扮类会自动补
        slot/dressup，无需手填。
      </p>
    </section>
  );
}
