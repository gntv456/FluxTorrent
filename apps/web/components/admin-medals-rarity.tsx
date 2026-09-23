"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { usableAssetUrl } from "@/components/medal-icon";
import { MedalRarityPreview } from "@/components/medal-rarity-chip";
import {
  toneList,
  cleanRarityValue,
  type MedalRarity,
} from "@/lib/medal-rarity";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 勋章管理的稀有度与预览部件（从 admin-medals.tsx 按域拆出）：
 *  RarityDict 稀有度词表编辑 + MedalImagePreview 图片预览。 */

/** 表单输入基线样式（原 admin-medals.tsx 的 RARITY_INP 常量） */
const RARITY_INP =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud " +
  "px-2 text-sm outline-none focus:border-sky";

/** 稀有度下拉里的「自定义…」哨兵值（真实值由旁边的文本框输入） */
export const CUSTOM_RARITY = "__custom__";

/** 稀有度词表编辑（0143）：键/显示名/配色档/排序；键即 medals.rarity 里存的值。
 *  有勋章在用的条目不许删（后端也拦），改键会连带迁移那些勋章。 */
export function RarityDict({
  rows,
  onChanged,
  flash,
}: {
  rows: MedalRarity[];
  onChanged: () => Promise<void> | void;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const at = dict.medalRarity;
  const TONES = toneList(at.tones);
  const blank = {
    key: null as string | null,
    value: "",
    label: "",
    tone: "sky",
    sort: 100,
  };
  const [edit, setEdit] = useState(blank);
  const [busy, setBusy] = useState(false);

  async function save() {
    const value = cleanRarityValue(edit.value);
    if (!value) return flash(at.keyEmpty);
    if (!edit.label.trim()) return flash(at.labelEmpty);
    setBusy(true);
    try {
      if (edit.key === null) {
        await api.post("/api/v1/admin/medal-rarities", {
          value,
          label: edit.label.trim(),
          tone: edit.tone,
          sort: edit.sort,
        });
      } else {
        await api.put(
          `/api/v1/admin/medal-rarities/${encodeURIComponent(edit.key)}`,
          {
            value,
            label: edit.label.trim(),
            tone: edit.tone,
            sort: edit.sort,
          },
        );
      }
      flash(at.saved);
      setEdit(blank);
      await onChanged();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex flex-wrap items-baseline gap-2">
        <h3 className="text-sm font-bold">{at.rarityTitle}</h3>
        <span className="text-[11px] text-sub">{at.rarityHint}</span>
      </div>
      <div className="mb-3 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {at.fKey}
          <input
            value={edit.value}
            onChange={(e) => setEdit({ ...edit, value: e.target.value })}
            placeholder={at.phKey}
            className={`${RARITY_INP} w-32`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fLabel}
          <input
            value={edit.label}
            onChange={(e) => setEdit({ ...edit, label: e.target.value })}
            placeholder={at.phLabel}
            className={`${RARITY_INP} w-28`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fTone}
          <select
            value={edit.tone}
            onChange={(e) => setEdit({ ...edit, tone: e.target.value })}
            className={RARITY_INP}
          >
            {TONES.map((t) => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fSort}
          <input
            type="number"
            value={edit.sort}
            onChange={(e) => setEdit({ ...edit, sort: Number(e.target.value) })}
            className={`${RARITY_INP} w-20`}
          />
        </label>
        <span className="pb-2">
          <MedalRarityPreview label={edit.label} tone={edit.tone} />
        </span>
        <button className="baozi-button" disabled={busy} onClick={save}>
          {edit.key === null ? at.add : at.save}
        </button>
        {edit.key !== null && (
          <button
            className={BTN_SM_BOLD}
            onClick={() => setEdit(blank)}
          >
            {at.cancel}
          </button>
        )}
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thKey}</td>
            <td className="colhead">{at.thLabel}</td>
            <td className="colhead">{at.thTone}</td>
            <td className="colhead">{at.thSort}</td>
            <td className="colhead">{at.thUsed}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.value}>
              <td className="font-mono">{r.value}</td>
              <td>{r.label}</td>
              <td>
                <MedalRarityPreview label={r.label} tone={r.tone} />
              </td>
              <td className="num">{r.sort ?? 0}</td>
              <td className="num">{r.used ?? 0}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEdit({
                      key: r.value,
                      value: r.value,
                      label: r.label,
                      tone: r.tone,
                      sort: r.sort ?? 100,
                    })
                  }
                >
                  {at.edit}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy || (r.used ?? 0) > 0}
                  title={
                    (r.used ?? 0) > 0
                      ? fmt(at.usedTitle, { n: r.used ?? 0 })
                      : at.delTitle
                  }
                  onClick={async () => {
                    try {
                      await api.del(
                        `/api/v1/admin/medal-rarities/${encodeURIComponent(r.value)}`,
                      );
                      flash(at.deleted);
                      await onChanged();
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
              <td colSpan={6} className="py-4 text-center text-sub">
                {at.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

/** 勋章图片预览：图挂了要说清原因（站内图床 /api/v1/attachments 需登录，<img> 拿不到） */
export function MedalImagePreview({ src }: { src?: string | null }) {
  const url = usableAssetUrl(src);
  const { dict } = useI18n();
  const at = dict.medalRarity;
  const [err, setErr] = useState(false);
  useEffect(() => {
    setErr(false);
  }, [url]);
  if (!url)
    return (
      <span className="pb-1 text-[11px] text-sub">{at.noImage}</span>
    );
  if (err)
    return (
      <span className="max-w-xs pb-1 text-[11px] text-danger">
        {at.imgFail}
      </span>
    );
  return (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={url}
      alt={at.imgAlt}
      className="h-10 w-10 rounded-[var(--r-sm)] border border-line bg-cloud object-cover"
      onError={() => setErr(true)}
    />
  );
}
