"use client";

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { FundingFormState, MineRow } from "@/components/funding-panel";

/** 定向众筹免费展示件（从 funding-panel.tsx 按域拆出）：
 *  发起表单 + 状态标签 + 我的参与表格（数据装载与动作留在原文件）。 */

/** 状态 → 文案（0 进行中 / 1 已达成 / 2 已退款 / 3 已了结） */
export function statusLabel(
  s: number,
  t: {
    stActive: string;
    stReached: string;
    stRefunded: string;
    stSettled: string;
    stOther: string;
  },
) {
  return s === 0
    ? t.stActive
    : s === 1
      ? t.stReached
      : s === 2
        ? t.stRefunded
        : s === 3
          ? t.stSettled
          : t.stOther;
}

/** 我的参与（GET /fundings/mine 元组表格） */
export function MineTable({
  mine,
  statusOf,
}: {
  mine: MineRow[] | null;
  statusOf: (s: number) => string;
}) {
  const { dict } = useI18n();
  const t = dict.funding;
  return (
    <div>
      <h3 className="mb-2 text-sm font-bold">{t.mine}</h3>
      <div className="baozi-wide-table-scroll">
      <table className="nexus-table text-xs">
        <tbody>
          <tr>
            <td className="colhead">#</td>
            <td className="colhead">{t.raisedCol}</td>
            <td className="colhead w-24">{t.statusCol}</td>
          </tr>
          {(mine ?? []).map((m, i) => (
            <tr key={`${m[0]}-${i}`}>
              <td className="rowfollow num">
                <a className="text-link" href={`/torrents`}>
                  #{m[0]}
                </a>
              </td>
              <td className="rowfollow num">
                {fmt(t.minePaid, {
                  paid: m[1].toLocaleString(),
                  tax: m[2].toLocaleString(),
                })}
              </td>
              <td className="rowfollow">{statusOf(m[3])}</td>
            </tr>
          ))}
          {mine !== null && mine.length === 0 && (
            <tr>
              <td colSpan={3} className="py-4 text-center text-sub">
                {t.mineEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      </div>
    </div>
  );
}

/** 发起众筹表单（发布者或 staff；提交动作由父层 onCreate 传入） */
export function CreateForm({
  form,
  setForm,
  busy,
  inputCls,
  onCreate,
}: {
  form: FundingFormState;
  setForm: (f: FundingFormState) => void;
  busy: boolean;
  inputCls: string;
  onCreate: () => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.funding;
  return (
    <form
      className="flex flex-col gap-2 rounded-[var(--r-md)] border border-dashed border-line p-3"
      onSubmit={(e) => {
        e.preventDefault();
        onCreate();
      }}
    >
      <p className="text-xs font-bold text-sub">{t.createTitle}</p>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {t.fldTorrent}
          <input
            type="number"
            min={1}
            required
            value={form.torrent_id}
            onChange={(e) => setForm({ ...form, torrent_id: e.target.value })}
            className={`${inputCls} w-28`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fldGoal.replaceAll("{magic}", currency)}
          <input
            type="number"
            min={1000}
            required
            value={form.goal}
            onChange={(e) => setForm({ ...form, goal: e.target.value })}
            className={`${inputCls} w-36`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fldHours}
          <input
            type="number"
            min={1}
            max={720}
            value={form.hours}
            onChange={(e) => setForm({ ...form, hours: e.target.value })}
            className={`${inputCls} w-40`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fldDays}
          <input
            type="number"
            min={1}
            max={60}
            value={form.days}
            onChange={(e) => setForm({ ...form, days: e.target.value })}
            className={`${inputCls} w-32`}
          />
        </label>
        <button
          type="submit"
          disabled={busy || !form.torrent_id}
          className="baozi-button min-h-[38px] text-xs disabled:opacity-50"
        >
          {t.createBtn}
        </button>
      </div>
    </form>
  );
}
