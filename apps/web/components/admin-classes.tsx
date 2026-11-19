"use client";

import { BTN_SM_GHOST, INPUT_MD } from "@/lib/ui-classes";

import { Fragment, useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { ImpactPreviewCard } from "./admin-classes-preview";
import { ClassNumCell, ClassOptNumCell } from "./admin-classes-fields";
import type { ImpactPreview } from "./admin-classes-preview";

/** P-19：等级规则管理页（读写口此前只有 API 没有界面）。
 *  worker 每分钟按阈值升降级并发 promo_sparks；成长档 1-12 有规则行，
 *  贵宾/职务档无规则整行只读。编辑原语在 admin-classes-fields.tsx。 */

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
  /** 0501 四维：null = 不参与判定 */
  min_downloaded: number | null;
  min_seed_volume: number | null;
  min_seed_count: number | null;
  /** 万分比（125 = 1.25） */
  min_ratio_bp: number | null;
  users: number;
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
  /** 0501 四维：null = 清除该维度（不参与判定）；字节维度界面按 GB */
  min_downloaded: number | null;
  min_seed_volume: number | null;
  min_seed_count: number | null;
  min_ratio_bp: number | null;
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
    min_downloaded: r.min_downloaded,
    min_seed_volume: r.min_seed_volume,
    min_seed_count: r.min_seed_count,
    min_ratio_bp: r.min_ratio_bp,
  };

  const patch = (
    r: ClassRow,
    k: keyof Draft,
    v: number | string | boolean | null,
  ) => {
    setDrafts((prev) => ({
      ...prev,
      [r.class_id]: { ...draftOf(r), [k]: v },
    }));
    // 草稿一改旧预览即失效（计数对不上新阈值）
    setPreview((p) => ({ ...p, [r.class_id]: null }));
  };

  /** 预览影响：按草稿阈值问后端「改完升降多少人」，名单最多 10 个样例 */
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

  /** 无规则行/职务档只读 */
  const locked = (r: ClassRow) =>
    r.min_uploaded === null || r.class_id >= STAFF_FLOOR;

  /** 表格编辑原语的上下文（绑定行 + 草稿键；原语件只管渲染与量纲） */
  const ctx = (
    r: ClassRow,
    k: keyof Draft,
  ) => ({
    locked: locked(r),
    get: () => draftOf(r)[k] as number | null,
    set: (v: number | null) => patch(r, k, v),
    setLegacy: (v: number) => patch(r, k, v),
  });



  return (
    <div className="flex flex-col gap-3">
      <div>
        <h2 className="text-base font-bold text-ink">{at.title}</h2>
        <p className="text-sm text-sub">{at.intro}</p>
      </div>
      {msg && (
        <p
          className={`
            rounded-[var(--r-md)] bg-[var(--surface-sunken)]
            p-3 text-sm
          `}
        >
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
              <th>{at.thDlBytes}</th>
              <th>{at.thSeedVol}</th>
              <th>{at.thSeedCnt}</th>
              <th>{at.thRatio}</th>
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
                  <td>{<ClassNumCell ctx={ctx(r, "min_uploaded")} scale={GB}
                      unit={at.unitGB} />}</td>
                  <td>{<ClassNumCell ctx={ctx(r, "min_download_count")}
                      unit={at.unitCount} />}</td>
                  <td>{<ClassNumCell ctx={ctx(r, "min_seed_hours")}
                      unit={at.unitHours} />}</td>
                  <td>{<ClassNumCell ctx={ctx(r, "min_account_age_days")}
                      unit={at.unitDays} />}</td>
                  <td>{<ClassOptNumCell ctx={ctx(r, "min_downloaded")}
                      scale={GB} unit={at.unitGB} />}</td>
                  <td>{<ClassOptNumCell ctx={ctx(r, "min_seed_volume")}
                      scale={GB} unit={at.unitGB} />}</td>
                  <td>{<ClassOptNumCell ctx={ctx(r, "min_seed_count")}
                      unit={at.unitCount} />}</td>
                  <td>{<ClassOptNumCell ctx={ctx(r, "min_ratio_bp")}
                      scale={10000} unit={at.unitRatio} />}</td>
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
                    <ClassNumCell ctx={ctx(r, "promo_sparks")} />
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
                {pv && <ImpactPreviewCard pv={pv} at={at} />}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
