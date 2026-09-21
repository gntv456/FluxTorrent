"use client";

import type { MedalRarity } from "@/lib/medal-rarity";
import type { MedalRow } from "./admin-medals-shared";
import { CUSTOM_RARITY, MedalImagePreview } from "./admin-medals-rarity";

/** 勋章新建/编辑表单（从 admin-medals.tsx 按域拆出）。 */
export function MedalForm(props: {
  edit: { id: number | null; f: Partial<MedalRow> };
  setEdit: React.Dispatch<
    React.SetStateAction<{ id: number | null; f: Partial<MedalRow> }>
  >;
  rarities: MedalRarity[];
  customRarity: boolean;
  setCustomRarity: (v: boolean) => void;
  rarityIsCustom: boolean;
  save: () => void;
  resetForm: () => void;
  busy: boolean;
  inp: string;
  currency: string;
  asetBar: string;
  clearBtn: string;
  plainBtn: string;
}) {
  const {
    edit,
    setEdit,
    rarities,
    customRarity,
    setCustomRarity,
    rarityIsCustom,
    save,
    resetForm,
    busy,
    inp,
    currency,
  } = props;
  const ASSET_BAR_CLS = props.asetBar;
  const CLEAR_BTN_CLS = props.clearBtn;
  const PLAIN_BTN_CLS = props.plainBtn;
  return (
    <section className="baozi-panel cmgmt-form p-4">
      <h2 className="mb-2 text-base font-bold">
        {edit.id === null ? "新建勋章" : `编辑勋章 #${edit.id}`}
      </h2>
      {/* 勋章图片（medals.asset_ref）：此前后端能存、表单没入口 → 全站只能画 🏅 */}
      <div className={ASSET_BAR_CLS}>
        <label className="flex flex-col gap-1 text-xs">
          勋章图片 URL
          <input
            value={edit.f.asset_ref ?? ""}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, asset_ref: e.target.value || null },
              })
            }
            placeholder="https://…/medal.png"
            className={`${inp} w-80`}
          />
        </label>
        <MedalImagePreview src={edit.f.asset_ref} />
        {edit.f.asset_ref ? (
          <button
            type="button"
            className={CLEAR_BTN_CLS}
            onClick={() =>
              setEdit({ ...edit, f: { ...edit.f, asset_ref: null } })
            }
          >
            清除图片
          </button>
        ) : null}
      </div>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          名称
          <input
            value={edit.f.name ?? ""}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
            }
            className={`${inp} w-32`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          说明
          <input
            value={edit.f.description ?? ""}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, description: e.target.value },
              })
            }
            className={`${inp} w-48`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          获取方式
          <select
            value={edit.f.get_type ?? 2}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, get_type: Number(e.target.value) },
              })
            }
            className={inp}
          >
            <option value={1}>兑换</option>
            <option value={2}>授予</option>
            <option value={3}>合成</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          价格({currency})
          <input
            type="number"
            value={edit.f.price ?? ""}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: {
                  ...edit.f,
                  price: e.target.value ? Number(e.target.value) : null,
                },
              })
            }
            className={`${inp} w-24`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          魔力加成(%)
          <input
            type="number"
            value={edit.f.bonus_addition_factor ?? ""}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: {
                  ...edit.f,
                  bonus_addition_factor: e.target.value
                    ? Number(e.target.value)
                    : null,
                },
              })
            }
            className={`${inp} w-24`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          有效期(天,空=永久)
          <input
            type="number"
            value={edit.f.duration_days ?? ""}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: {
                  ...edit.f,
                  duration_days: e.target.value ? Number(e.target.value) : null,
                },
              })
            }
            className={`${inp} w-24`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          稀有度
          <select
            value={rarityIsCustom ? CUSTOM_RARITY : (edit.f.rarity ?? "")}
            onChange={(e) => {
              const v = e.target.value;
              if (v === CUSTOM_RARITY) {
                setCustomRarity(true);
                setEdit({ ...edit, f: { ...edit.f, rarity: null } });
              } else {
                setCustomRarity(false);
                setEdit({ ...edit, f: { ...edit.f, rarity: v || null } });
              }
            }}
            className={inp}
          >
            <option value="">未设置</option>
            {rarities.map((r) => (
              <option
                key={r.value}
                value={r.value}
              >{`${r.label}（${r.value}）`}</option>
            ))}
            <option value={CUSTOM_RARITY}>自定义…</option>
          </select>
        </label>
        {rarityIsCustom && (
          <label className="flex flex-col gap-1 text-xs">
            自定义稀有度
            <input
              value={edit.f.rarity ?? ""}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, rarity: e.target.value || null },
                })
              }
              placeholder="如 super-rare"
              className={`${inp} w-32`}
            />
          </label>
        )}
        <label className="flex flex-col gap-1 text-xs">
          分组
          <input
            type="number"
            value={edit.f.category_id ?? 0}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, category_id: Number(e.target.value) },
              })
            }
            className={`${inp} w-16`}
          />
        </label>
        <label className="flex items-center gap-1 pb-2 text-xs">
          <input
            type="checkbox"
            checked={Boolean(edit.f.limited)}
            onChange={(e) =>
              setEdit({
                ...edit,
                f: { ...edit.f, limited: e.target.checked },
              })
            }
          />
          限定
        </label>
        <button
          className="baozi-button"
          disabled={busy || !String(edit.f.name ?? "").trim()}
          onClick={save}
        >
          保存
        </button>
        {edit.id !== null && (
          <button className={PLAIN_BTN_CLS} onClick={resetForm}>
            取消
          </button>
        )}
      </div>
    </section>
  );
}
