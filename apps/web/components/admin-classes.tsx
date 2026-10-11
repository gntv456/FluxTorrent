"use client";

import { BTN_SM_GHOST, INPUT_MD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** P-19：等级规则管理页（一审 R4.2 落的读写口，此前只有 API 没有界面）。
 *
 *  worker `class_auto_adjust` 每分钟按这里的阈值升降级，并按
 *  `promo_sparks` 发放晋升奖励；奖励路径（0072）实现完整却因无界面而
 *  常年 0 值空转——本页就是把这条链接给站长的入口。
 *  staff 档（class_id >= 90）后端拒绝编辑（403/校验），前端一并只读。 */

const GB = 1024 ** 3;
/** 后端锁死的 staff 线（classes_admin.rs STAFF_CLASS_FLOOR） */
const STAFF_FLOOR = 90;

interface ClassRow {
  class_id: number;
  name: string;
  min_uploaded: number;
  min_download_count: number;
  min_seed_hours: number;
  min_account_age_days: number;
  demotable: boolean;
  promo_sparks: number;
  users: number;
}

/** 草稿只存被改动的档位；字节门槛在界面上按 GB 编辑 */
type Draft = Pick<
  ClassRow,
  | "name"
  | "min_uploaded"
  | "min_download_count"
  | "min_seed_hours"
  | "min_account_age_days"
  | "demotable"
  | "promo_sparks"
>;

export function AdminClasses() {
  const { dict } = useI18n();
  const at = dict.adminClasses;
  const [rows, setRows] = useState<ClassRow[]>([]);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<number | null>(null);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<ClassRow[]>("/api/v1/admin/classes"));
      setDrafts({});
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.loadFail);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const draftOf = (r: ClassRow): Draft => drafts[r.class_id] ?? {
    name: r.name,
    min_uploaded: r.min_uploaded,
    min_download_count: r.min_download_count,
    min_seed_hours: r.min_seed_hours,
    min_account_age_days: r.min_account_age_days,
    demotable: r.demotable,
    promo_sparks: r.promo_sparks,
  };

  const patch = (r: ClassRow, k: keyof Draft, v: number | string | boolean) =>
    setDrafts((prev) => ({ ...prev, [r.class_id]: { ...draftOf(r), [k]: v } }));

  async function save(r: ClassRow) {
    const d = drafts[r.class_id];
    if (!d) return;
    setBusy(r.class_id);
    try {
      await api.put(`/api/v1/admin/classes/${r.class_id}`, d);
      flash(fmt(at.savedMsg, { n: r.class_id }));
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.saveFail);
    } finally {
      setBusy(null);
    }
  }

  const num = (
    r: ClassRow, k: keyof Draft, scale = 1, disabled = false,
  ) => (
    <input
      type="number"
      className={`${INPUT_MD} w-24`}
      value={Number(draftOf(r)[k]) / scale}
      disabled={disabled}
      onChange={(e) =>
        patch(r, k, Math.round(Number(e.target.value) * scale))}
    />
  );

  return (
    <div className="flex flex-col gap-3">
      <div>
        <h2 className="text-base font-bold text-ink">{at.title}</h2>
        <p className="text-sm text-sub">{at.intro}</p>
      </div>
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-sub">
              <th>{at.thTier}</th>
              <th>{at.thName}</th>
              <th>{at.thUp}</th>
              <th>{at.thDl}</th>
              <th>{at.thSeedH}</th>
              <th>{at.thAge}</th>
              <th>{at.thDemote}</th>
              <th>{at.thSparks}</th>
              <th className="text-right">{at.thUsers}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const locked = r.class_id >= STAFF_FLOOR;
              const d = draftOf(r);
              return (
                <tr
                  key={r.class_id}
                  className="border-t border-line align-middle"
                >
                  <td>{r.class_id}</td>
                  <td>
                    <input
                      className={`${INPUT_MD} w-32`}
                      value={d.name}
                      disabled={locked}
                      onChange={(e) => patch(r, "name", e.target.value)}
                    />
                  </td>
                  <td>
                    {num(r, "min_uploaded", GB, locked)}
                    <span className="ml-1 text-xs text-sub">{at.upHint}</span>
                  </td>
                  <td>{num(r, "min_download_count", 1, locked)}</td>
                  <td>{num(r, "min_seed_hours", 1, locked)}</td>
                  <td>{num(r, "min_account_age_days", 1, locked)}</td>
                  <td>
                    <input
                      type="checkbox"
                      checked={d.demotable}
                      disabled={locked}
                      title={at.demoteHint}
                      onChange={(e) =>
                        patch(r, "demotable", e.target.checked)}
                    />
                  </td>
                  <td>
                    {num(r, "promo_sparks", 1, locked)}
                    <div className="text-xs text-sub">{at.sparksHint}</div>
                  </td>
                  <td className="text-right">{r.users}</td>
                  <td className="text-right">
                    {locked ? (
                      <span className="text-xs text-sub">
                        {at.staffLocked}
                      </span>
                    ) : (
                      <button
                        type="button"
                        className={BTN_SM_GHOST}
                        disabled={busy === r.class_id || !drafts[r.class_id]}
                        onClick={() => save(r)}
                      >
                        {at.save}
                      </button>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
