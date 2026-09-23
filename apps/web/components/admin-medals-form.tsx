"use client";

import type { MedalRarity } from "@/lib/medal-rarity";
import type { MedalRow } from "./admin-medals-shared";
import { CUSTOM_RARITY, MedalImagePreview } from "./admin-medals-rarity";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

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
  const { dict } = useI18n();
  const at = dict.adminMedals;
  const ASSET_BAR_CLS = props.asetBar;
  const CLEAR_BTN_CLS = props.clearBtn;
  const PLAIN_BTN_CLS = props.plainBtn;
  return (
    <section className="baozi-panel cmgmt-form p-4">
      <h2 className="mb-2 text-base font-bold">
        {edit.id === null
          ? at.formNew
          : fmt(at.formEdit, { id: edit.id })}
      </h2>
      {/* 勋章图片（medals.asset_ref）：此前后端能存、表单没入口 → 全站只能画 🏅 */}
      <div className={ASSET_BAR_CLS}>
        <label className="flex flex-col gap-1 text-xs">
          {at.fAsset}
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
            {at.clearAsset}
          </button>
        ) : null}
      </div>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {at.fName}
          <input
            value={edit.f.name ?? ""}
            onChange={(e) =>
              setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
            }
            className={`${inp} w-32`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fDescr}
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
          {at.fGetType}
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
            <option value={1}>{at.getType["1"]}</option>
            <option value={2}>{at.getType["2"]}</option>
            <option value={3}>{at.getType["3"]}</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {fmt(at.fPrice, { magic: currency })}
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
          {at.fBonus}
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
          {at.fDuration}
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
          {at.fRarity}
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
            <option value="">{at.rarityUnset}</option>
            {rarities.map((r) => (
              <option
                key={r.value}
                value={r.value}
              >{`${r.label}（${r.value}）`}</option>
            ))}
            <option value={CUSTOM_RARITY}>{at.rarityCustom}</option>
          </select>
        </label>
        {rarityIsCustom && (
          <label className="flex flex-col gap-1 text-xs">
            {at.fRarityCustom}
            <input
              value={edit.f.rarity ?? ""}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, rarity: e.target.value || null },
                })
              }
              placeholder={at.phRarityCustom}
              className={`${inp} w-32`}
            />
          </label>
        )}
        <label className="flex flex-col gap-1 text-xs">
          {at.fGroup}
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
          {at.limited}
        </label>
        <button
          className="baozi-button"
          disabled={busy || !String(edit.f.name ?? "").trim()}
          onClick={save}
        >
          {at.save}
        </button>
        {edit.id !== null && (
          <button className={PLAIN_BTN_CLS} onClick={resetForm}>
            {at.cancel}
          </button>
        )}
      </div>
    </section>
  );
}
