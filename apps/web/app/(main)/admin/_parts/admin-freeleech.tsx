export { ClearCachePanel } from "./admin-clearcache";

"use client";

import { BTN_MD_SKY, INPUT_CLOUD } from "@/lib/ui-classes";

/**
 * 管理后台促销状态面板（从 app/(main)/admin/page.tsx 按域拆出，
 * 后并回 ./_parts/admin-tool-panels.tsx 家族）：FreeleechPanel 对应
 * /admin/freeleech 的增删查。清除缓存面板同文件承载。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { dateLocale, fmt } from "@/i18n/config";
import type { PromoRow } from "./admin-shared";

/** 促销取值键（显示名在 i18n adminPromo.kinds / .scopes，取不到回落原值） */
const PROMO_KIND_KEYS = ["free", "x2", "x2free", "half", "x2half", "p30"];
const PROMO_SCOPE_KEYS = ["global", "official", "non_official", "category"];

/** 促销状态（freeleech）管理：对应 /admin/freeleech 的增删查 */
export function FreeleechPanel() {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const t = dict.adminPromo;
  const [rows, setRows] = useState<PromoRow[]>([]);
  const [kind, setKind] = useState("free");
  const [scope, setScope] = useState("global");
  const [hours, setHours] = useState(24);
  // 0503 预约 UI：API 一直支持 starts_at/ends_at（RFC3339，缺省=now）——
  // 表单此前只暴露 hours，站长想规划活动只能调 API
  const [startsAt, setStartsAt] = useState("");
  const [categoryId, setCategoryId] = useState("1");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // P1：全站促销直接影响经济，创建/移除需二次确认
  const [ask, setAsk] = useState<
    { type: "create" } | { type: "remove"; id: number } | null
  >(null);

  const load = useCallback(async () => {
    try {
      setRows(await api.get<PromoRow[]>("/api/v1/admin/freeleech"));
    } catch {
      setRows([]); setMsg(dict.common.loadFailed);
    }
  }, [dict]);

  useEffect(() => {
    load();
  }, [load]);

  async function create() {
    setBusy(true);
    try {
      await api.post("/api/v1/admin/freeleech", {
        kind,
        scope,
        hours,
        ...(startsAt ? { starts_at: new Date(startsAt).toISOString() } : {}),
        ...(scope === "category" ? { category_id: Number(categoryId) } : {}),
      });
      setMsg(t.applied);
      load();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (apiErrorMessage(dict, e))
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: number) {
    setBusy(true);
    try {
      await api.del(`/api/v1/admin/freeleech/${id}`);
      setMsg(t.removed);
      load();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (apiErrorMessage(dict, e))
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    INPUT_CLOUD;

  return (
    <section className="brand-panel p-4">
      <h2 className="mb-2 text-base font-bold">{t.title}</h2>
      <p className="mb-3 text-xs text-sub">{t.intro}</p>
      {msg && (
        <p
          className={
            "mb-3 rounded-[var(--r-md)] bg-[var(--surface-sunken)] " +
            "p-2 text-xs"
          }
        >
          {msg}
        </p>
      )}
      <div className="brand-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <th scope="col" className="colhead w-24">{t.colKind}</th>
              <th scope="col" className="colhead w-28">{t.colScope}</th>
              <th scope="col" className="colhead">{t.colStart}</th>
              <th scope="col" className="colhead">{t.colEnd}</th>
              <td className="colhead w-20" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="rowfollow">
                  {t.kinds[r.kind] ?? r.kind}
                </td>
                <td className="rowfollow">
                  {t.scopes[r.scope] ?? r.scope}
                  {r.category_name ? ` · ${r.category_name}` : ""}
                </td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(r.starts_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(r.ends_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="rowfollow">
                  <button
                    disabled={busy}
                    onClick={() => setAsk({ type: "remove", id: r.id })}
                    className={
                      "min-h-11 rounded-full border border-line px-3 " +
                      "font-bold text-[var(--danger-text)] disabled:opacity-50"
                    }
                  >
                    {t.removeBtn}
                  </button>
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={5} className="py-4 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-4 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldKind}</span>
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            className={inputCls}
          >
            {PROMO_KIND_KEYS.map((k) => (
              <option key={k} value={k}>
                {t.kinds[k] ?? k}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldScope}</span>
          <select
            value={scope}
            onChange={(e) => setScope(e.target.value)}
            className={inputCls}
          >
            {PROMO_SCOPE_KEYS.map((s) => (
              <option key={s} value={s}>
                {t.scopes[s] ?? s}
              </option>
            ))}
          </select>
        </label>
        {scope === "category" && (
          <label className="flex flex-col gap-1">
            <span className="text-xs text-sub">{t.fldCategory}</span>
            <input
              type="number"
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
              className={`${inputCls} w-24`}
            />
          </label>
        )}
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldHours}</span>
          <input
            type="number"
            min={1}
            max={720}
            value={hours}
            onChange={(e) => setHours(Number(e.target.value))}
            className={`${inputCls} w-28`}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldStartsAt}</span>
          <input
            type="datetime-local"
            value={startsAt}
            onChange={(e) => setStartsAt(e.target.value)}
            className={`${inputCls} w-56`}
          />
          <span className="text-[10px] opacity-60">{t.fldStartsHint}</span>
        </label>
        <button
          disabled={busy}
          onClick={() => setAsk({ type: "create" })}
          className={BTN_MD_SKY}
        >
          {t.applyBtn}
        </button>
      </div>
      <ConfirmDialog
        open={ask !== null}
        onClose={() => setAsk(null)}
        onConfirm={() => {
          const a = ask;
          setAsk(null);
          if (!a) return;
          if (a.type === "create") void create();
          else void remove(a.id);
        }}
        tone="danger"
        title={dict.review.confirmFreeleechTitle}
        message={fmt(dict.review.confirmFreeleechBody, {
          scope,
          hours: String(hours),
        })}
        busy={busy}
      />
    </section>
  );
}
