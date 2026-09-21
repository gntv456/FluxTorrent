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
import { GET_TYPE } from "./admin-medals-shared";

/** 第八轮 P2-7：勋章管理（好学站 system/medals 简化口径）
 *  字典 CRUD + 全站持有浏览 + 回收（授予入口在用户详情页）。
 *  稀有度词表编辑与图片预览拆出 admin-medals-rarity.tsx；
 *  持有浏览拆至 ./admin-medals-held.tsx；类型拆至
 *  ./admin-medals-shared.ts（300 门禁）。 */

/** 勋章图片行容器 */
const ASSET_BAR_CLS =
  "mb-3 flex flex-wrap items-center gap-3 border-b border-line pb-3";
/** 清除图片小按钮 */
const CLEAR_BTN_CLS =
  BTN_XS_GHOST;
/** 圆角描边小按钮（取消） */
const PLAIN_BTN_CLS =
  BTN_SM_BOLD;
/** 「限定」徽标 */
const LIMITED_CHIP_CLS =
  "ml-1 rounded-full bg-coral/20 px-1.5 text-[10px] text-danger";

export function AdminMedals() {
  const { currency } = useI18n();
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
      flash(e instanceof ApiError ? e.message : "加载失败");
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
      flash("已保存");
      resetForm();
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud " +
    "px-2 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

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
                    duration_days: e.target.value
                      ? Number(e.target.value)
                      : null,
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

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">图标</td>
            <td className="colhead">名称</td>
            <td className="colhead">获取</td>
            <td className="colhead">稀有度</td>
            <td className="colhead">价格</td>
            <td className="colhead">加成%</td>
            <td className="colhead">有效期</td>
            <td className="colhead">持有数</td>
            <td className="colhead text-right">操作</td>
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
                {m.limited && <span className={LIMITED_CHIP_CLS}>限定</span>}
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
              <td className="num">{m.duration_days ?? "永久"}</td>
              <td className="num">{m.held_count}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() => {
                    setEdit({ id: m.id, f: { ...m } });
                    setCustomRarity(false);
                  }}
                >
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/medals/${m.id}`);
                      flash("已删除");
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "删除失败");
                    }
                  }}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={10} className="py-6 text-center text-sub">
                暂无勋章
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
  );
}
