"use client";

import { BTN_SM_GHOST, INPUT_MD } from "@/lib/ui-classes";

import { Fragment, useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** P-19：等级规则管理页（一审 R4.2 落的读写口，此前只有 API 没有界面）。
 *  worker `class_auto_adjust` 每分钟按这里的阈值升降级，并按
 *  `promo_sparks` 发放晋升奖励（0072）；档位全集来自 user_classes：
 *  只有成长档 1-12 在 class_rules 里有规则行，贵宾（20）与职务档
 *  （90+）没有规则，六列为 null，整行只读。 */

const GB = 1024 ** 3;
/** 后端锁死的 staff 线（classes_admin.rs STAFF_CLASS_FLOOR） */
const STAFF_FLOOR = 90;

interface ClassRow {
  class_id: number;
  name: string;
  min_uploaded: number | null;
  min_download_count: number | null;
  min_seed_hours: number | null;
  min_account_age_days: number | null;
  demotable: boolean | null;
  promo_sparks: number | null;
  users: number;
}

/** POST impact-preview 的返回（按草稿阈值预览升降集合） */
interface ImpactPreview {
  promote: number;
  demote: number;
  sample_promote: string[];
  sample_demote: string[];
  current_users: number;
}

/** 草稿只存被改动的档位；字节门槛在界面上按 GB 编辑。
 *  六列在 ClassRow 里可空（只读档无规则），进草稿前一律补 0，
 *  免得 null 落进 PUT body 被后端类型校验打回。 */
type Draft = {
  name: string;
  min_uploaded: number;
  min_download_count: number;
  min_seed_hours: number;
  min_account_age_days: number;
  demotable: boolean;
  promo_sparks: number;
};

export function AdminClasses() {
  const { dict } = useI18n();
  const at = dict.adminClasses;
  const [rows, setRows] = useState<ClassRow[]>([]);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const [previewBusy, setPreviewBusy] = useState<number | null>(null);
  /** 按档位暂存的预览结果（草稿一改即失效） */
  const [preview, setPreview] = useState<Record<number, ImpactPreview | null>>(
    {},
  );

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
    min_uploaded: r.min_uploaded ?? 0,
    min_download_count: r.min_download_count ?? 0,
    min_seed_hours: r.min_seed_hours ?? 0,
    min_account_age_days: r.min_account_age_days ?? 0,
    demotable: r.demotable ?? false,
    promo_sparks: r.promo_sparks ?? 0,
  };

  const patch = (r: ClassRow, k: keyof Draft, v: number | string | boolean) => {
    setDrafts((prev) => ({
      ...prev,
      [r.class_id]: { ...draftOf(r), [k]: v },
    }));
    // 草稿一改，旧预览立即失效（计数对不上新阈值，留着只会误导）
    setPreview((p) => ({ ...p, [r.class_id]: null }));
  };

  /** 预览影响（后端只读端点）：按当前草稿的四维 + demotable
   *  假设值问 worker「改完会升降多少人」，名单最多回 10 个样例 */
  async function runPreview(r: ClassRow) {
    const d = drafts[r.class_id] ?? draftOf(r);
    setPreviewBusy(r.class_id);
    try {
      const p = await api.post<ImpactPreview>(
        `/api/v1/admin/classes/${r.class_id}/impact-preview`,
        {
          min_uploaded: d.min_uploaded,
          min_download_count: d.min_download_count,
          min_seed_hours: d.min_seed_hours,
          min_account_age_days: d.min_account_age_days,
          demotable: d.demotable,
        },
      );
      setPreview((prev) => ({ ...prev, [r.class_id]: p }));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.previewFail);
    } finally {
      setPreviewBusy(null);
    }
  }

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

  /** 无规则行（贵宾/职务档）或后端 staff 线以下才允许编辑 */
  const locked = (r: ClassRow) =>
    r.min_uploaded === null || r.class_id >= STAFF_FLOOR;

  const num = (r: ClassRow, k: keyof Draft, scale = 1) => (
    <input
      type="number"
      className={`${INPUT_MD} w-24`}
      value={Number(draftOf(r)[k]) / scale}
      onChange={(e) =>
        patch(r, k, Math.round(Number(e.target.value) * scale))}
    />
  );

  /** 只读档的四维/奖励显示「—」而不是 0：0 会被读成「门槛为零」 */
  const cell = (r: ClassRow, k: keyof Draft, scale = 1) =>
    locked(r) ? <span className="text-xs text-sub">—</span> : num(r, k, scale);

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
              const d = draftOf(r);
              const pv = preview[r.class_id];
              return (
                <Fragment key={r.class_id}>
                <tr className="border-t border-line align-middle">
                  <td>{r.class_id}</td>
                  <td>
                    {locked(r) ? (
                      r.name
                    ) : (
                      <input
                        className={`${INPUT_MD} w-32`}
                        value={d.name}
                        onChange={(e) => patch(r, "name", e.target.value)}
                      />
                    )}
                  </td>
                  <td>
                    {cell(r, "min_uploaded", GB)}
                    {!locked(r) && (
                      <span className="ml-1 text-xs text-sub">
                        {at.upHint}
                      </span>
                    )}
                  </td>
                  <td>{cell(r, "min_download_count")}</td>
                  <td>{cell(r, "min_seed_hours")}</td>
                  <td>{cell(r, "min_account_age_days")}</td>
                  <td>
                    {locked(r) ? (
                      <span className="text-xs text-sub">—</span>
                    ) : (
                      <input
                        type="checkbox"
                        checked={d.demotable}
                        title={at.demoteHint}
                        onChange={(e) =>
                          patch(r, "demotable", e.target.checked)}
                      />
                    )}
                  </td>
                  <td>
                    {cell(r, "promo_sparks")}
                    {!locked(r) && (
                      <div className="text-xs text-sub">{at.sparksHint}</div>
                    )}
                  </td>
                  <td className="text-right">{r.users}</td>
                  <td className="text-right">
                    {locked(r) ? (
                      <span className="text-xs text-sub">
                        {at.staffLocked}
                      </span>
                    ) : (
                      <div className="flex justify-end gap-1">
                        <button
                          type="button" className={BTN_SM_GHOST}
                          disabled={previewBusy === r.class_id}
                          onClick={() => runPreview(r)}
                        >
                          {previewBusy === r.class_id
                            ? at.previewBusy
                            : at.preview}
                        </button>
                        <button
                          type="button" className={BTN_SM_GHOST}
                          disabled={busy === r.class_id || !drafts[r.class_id]}
                          onClick={() => save(r)}
                        >
                          {at.save}
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
                {pv && (
                  <tr className="border-t border-line">
                    <td colSpan={10}>
                      <div
                        className="rounded-[var(--r-md)] bg-sky-soft p-3
                          text-xs text-ink"
                      >
                        <p>
                          {fmt(at.pvPromote, { n: pv.promote })} ·{" "}
                          {fmt(at.pvDemote, { n: pv.demote })} ·{" "}
                          {fmt(at.pvCurrent, { n: pv.current_users })}
                        </p>
                        {pv.sample_promote.length > 0 && (
                          <p className="mt-1">{at.pvSampleUp}: {
                            pv.sample_promote.join(", ")}</p>
                        )}
                        {pv.sample_demote.length > 0 && (
                          <p className="mt-1">{at.pvSampleDown}: {
                            pv.sample_demote.join(", ")}</p>
                        )}
                      </div>
                    </td>
                  </tr>
                )}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
