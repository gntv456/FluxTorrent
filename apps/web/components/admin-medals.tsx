"use client";

import { BTN_SM_BOLD, BTN_XS_GHOST } from "@/lib/ui-classes";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { MedalIcon } from "@/components/medal-icon";
import { MedalRarityChip } from "@/components/medal-rarity-chip";
import {
  DEFAULT_MEDAL_RARITIES,
  isKnownRarity,
  type MedalRarity,
} from "@/lib/medal-rarity";
import {
  CUSTOM_RARITY,
  MedalImagePreview,
  RarityDict,
} from "@/components/admin-medals-rarity";
import { MedalHeldPanel } from "./admin-medals-held";
import type { MedalRow, UserMedalRow } from "./admin-medals-shared";
import { MedalForm } from "./admin-medals-form";
import { getTypeLabels } from "./admin-medals-shared";

/** 第八轮 P2-7：勋章管理（好学站 system/medals 简化口径）
 *  字典 CRUD + 全站持有浏览 + 回收（授予入口在用户详情页）。
 *  稀有度词表编辑与图片预览拆出 admin-medals-rarity.tsx；
 *  持有浏览拆至 ./admin-medals-held.tsx；类型拆至
 *  ./admin-medals-shared.ts（300 门禁）。 */

/** 勋章图片行容器 */
const ASSET_BAR_CLS =
  "mb-3 flex flex-wrap items-center gap-3 border-b border-line pb-3";
/** 清除图片小按钮 */
const CLEAR_BTN_CLS = BTN_XS_GHOST;
/** 圆角描边小按钮（取消） */
const PLAIN_BTN_CLS = BTN_SM_BOLD;
/** 「限定」徽标 */
const LIMITED_CHIP_CLS =
  "ml-1 rounded-full bg-coral/20 px-1.5 text-[10px] text-danger";

export function AdminMedals() {
  const { currency, dict } = useI18n();
  const at = dict.adminMedals;
  const GET_TYPE = getTypeLabels(at.getType);
  const [rows, setRows] = useState<MedalRow[]>([]);
  const [held, setHeld] = useState<UserMedalRow[]>([]);
  const [heldUid, setHeldUid] = useState("");
  const [edit, setEdit] = useState<{ id: number | null; f: Partial<MedalRow> }>(
    { id: null, f: { name: "", get_type: 2, category_id: 0, limited: false } },
  );
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** 稀有度选「自定义…」时为 true（DB 列是自由文本，允许站长新增稀有度） */
  const [customRarity, setCustomRarity] = useState(false);
  /** 稀有度词表（0143）：下拉候选与列表标签都读它 */
  const [rarities, setRarities] = useState<MedalRarity[]>(
    DEFAULT_MEDAL_RARITIES,
  );

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };
  const blankForm = {
    id: null,
    f: { name: "", get_type: 2, category_id: 0, limited: false },
  };
  const resetForm = () => {
    setEdit(blankForm);
    setCustomRarity(false);
  };
  // 已有值不在候选表里（历史数据 / 站长自定义）→ 自动落到自定义输入
  const rarityIsCustom =
    customRarity ||
    (!!edit.f.rarity && !isKnownRarity(rarities, edit.f.rarity));

  const load = useCallback(async () => {
    try {
      setRows(await api.get<MedalRow[]>("/api/v1/admin/medals"));
      // 词表拉不到就用内置兜底，不让整个面板报错
      setRarities(
        await api
          .get<MedalRarity[]>("/api/v1/admin/medal-rarities")
          .catch(() => DEFAULT_MEDAL_RARITIES),
      );
      const q = heldUid.trim()
        ? `?uid=${encodeURIComponent(heldUid.trim())}`
        : "";
      // 后端返回分页信封 {rows,total,page,per_page}（0096 前是裸数组，双形态兼容）
      const r = await api.get<UserMedalRow[] | { rows: UserMedalRow[] }>(
        `/api/v1/admin/user-medals${q}`,
      );
      setHeld(Array.isArray(r) ? r : r.rows);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.loadFail);
    }
  }, [heldUid]);
  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    setBusy(true);
    try {
      if (edit.id === null) await api.post("/api/v1/admin/medals", edit.f);
      else await api.put(`/api/v1/admin/medals/${edit.id}`, edit.f);
      flash(at.saved);
      resetForm();
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud " +
    "px-2 text-sm outline-none focus:border-sky";

  return (
    <>
      <MedalForm
        edit={edit}
        setEdit={setEdit}
        rarities={rarities}
        customRarity={customRarity}
        setCustomRarity={setCustomRarity}
        rarityIsCustom={rarityIsCustom}
        save={save}
        resetForm={resetForm}
        busy={busy}
        inp={inp}
        currency={currency}
        asetBar={ASSET_BAR_CLS}
        clearBtn={CLEAR_BTN_CLS}
        plainBtn={PLAIN_BTN_CLS}
      />
      <div className="flex flex-col gap-3">
        {msg && (
          <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
            {msg}
          </p>
        )}

        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{at.thId}</td>
              <td className="colhead">{at.thIcon}</td>
              <td className="colhead">{at.thName}</td>
              <td className="colhead">{at.thGetType}</td>
              <td className="colhead">{at.thRarity}</td>
              <td className="colhead">{at.thPrice}</td>
              <td className="colhead">{at.thBonus}</td>
              <td className="colhead">{at.thDuration}</td>
              <td className="colhead">{at.thHeld}</td>
              <td className="colhead text-right">{at.thAction}</td>
            </tr>
          </thead>
          <tbody>
            {rows.map((m) => (
              <tr key={m.id}>
                <td className="num">{m.id}</td>
                <td>
                  <MedalIcon src={m.asset_ref} size={28} title={m.name} />
                </td>
                <td className="font-bold">
                  {m.name}
                  {m.limited && (
                    <span className={LIMITED_CHIP_CLS}>
                      {at.limitedChip}
                    </span>
                  )}
                </td>
                <td>{GET_TYPE[m.get_type] ?? m.get_type}</td>
                <td>
                  {m.rarity ? (
                    <MedalRarityChip list={rarities} value={m.rarity} />
                  ) : (
                    "—"
                  )}
                </td>
                <td className="num">{m.price ?? "—"}</td>
                <td className="num">{m.bonus_addition_factor ?? 0}</td>
                <td className="num">{m.duration_days ?? at.forever}</td>
                <td className="num">{m.held_count}</td>
                <td className="text-right">
                  <button
                    className="cmgmt-act"
                    onClick={() => {
                      setEdit({ id: m.id, f: { ...m } });
                      setCustomRarity(false);
                    }}
                  >
                    {at.edit}
                  </button>
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={busy}
                    onClick={async () => {
                      try {
                        await api.del(`/api/v1/admin/medals/${m.id}`);
                        flash(at.deleted);
                        await load();
                      } catch (e) {
                        flash(e instanceof ApiError ? e.message : at.delFail);
                      }
                    }}
                  >
                    {at.del}
                  </button>
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={10} className="py-6 text-center text-sub">
                  {at.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>

        <RarityDict rows={rarities} onChanged={load} flash={flash} />

        {/* 持有浏览/回收（拆至 ./admin-medals-held.tsx） */}
        <MedalHeldPanel
          held={held}
          heldUid={heldUid}
          setHeldUid={setHeldUid}
          busy={busy}
          flash={flash}
          load={load}
        />
      </div>
    </>
  );
}
