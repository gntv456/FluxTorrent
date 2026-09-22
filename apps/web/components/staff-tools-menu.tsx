"use client";

import { BTN_MD_SKY, INPUT_CLOUD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 导航菜单面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  nexusphp-menu 口径——全局开关 + 位置/树形/等级/排序 CRUD。 */

interface MenuItemAdmin {
  id: number; location: string; label: string; url: string;
  parent_id: number; target: string; min_class: number; sort: number; enabled: boolean;
}
interface MenuAdminData { items: MenuItemAdmin[]; custom_enabled: boolean; min_visible_class: number }

export function StaffMenuPanel({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const t = dict.adminMenu;
  const [menuData, setMenuData] = useState<MenuAdminData | null>(null);
  const [mLoc, setMLoc] = useState("topbar");
  const [mLabel, setMLabel] = useState("");
  const [mUrl, setMUrl] = useState("");
  const [mParent, setMParent] = useState(0);
  const [mTarget, setMTarget] = useState("_self");
  const [mMinClass, setMMinClass] = useState(0);
  const [mSort, setMSort] = useState(0);
  const [mEnabled, setMEnabled] = useState(true);
  const [mEditId, setMEditId] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    api.get<MenuAdminData | null>("/api/v1/admin/menu-items").then(setMenuData).catch(() => setMenuData(null));
  }, []);
  useEffect(() => { load(); }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try { await fn(); flash(ok); await load(); }
    catch (e) { flash(e instanceof ApiError ? e.message : dict.common.networkError); }
    finally { setBusy(false); }
  }

  return (
    <section className="baozi-panel p-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-base font-bold">{t.title}</h2>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={menuData?.custom_enabled ?? false}
            onChange={(e) => guard(async () => {
              await api.put("/api/v1/admin/menu-settings", { custom_enabled: e.target.checked });
            }, e.target.checked ? t.enabledOn : t.enabledOff)}
          />
          {t.enableLabel}
        </label>
      </div>
      <p className="mb-3 text-xs text-sub">{t.intro}</p>
      <table className="nexus-table text-xs">
        <thead><tr>
          <td className="colhead w-20">{t.colLoc}</td>
          <td className="colhead">{t.colName}</td>
          <td className="colhead w-24">{t.colParent}</td>
          <td className="colhead w-16">{t.colClass}</td>
          <td className="colhead w-14">{t.colSort}</td>
          <td className="colhead w-14">{t.colStatus}</td>
          <td className="colhead w-32" />
        </tr></thead>
        <tbody>
          {(menuData?.items ?? []).map((m) => {
            const parent = (menuData?.items ?? []).find((x) => x.id === m.parent_id);
            return (
              <tr key={m.id} className={mEditId === m.id ? "bg-sky-soft" : ""}>
                <td>{m.location}</td>
                <td>
                  <span className="font-bold">{m.label}</span>
                  {m.target === "_blank" && <span className="ml-1 text-sub" title={t.newWindow}>↗</span>}
                  <p className="break-all text-sub">{m.url}</p>
                </td>
                <td className="text-sub">{parent ? parent.label : "—"}</td>
                <td className="num">{m.min_class}</td>
                <td className="num">{m.sort}</td>
                <td>{m.enabled ? t.statusOn : t.statusOff}</td>
                <td>
                  <button className="min-h-[28px] rounded-full border border-line px-3 font-bold text-sky"
                    onClick={() => {
                      if (mEditId === m.id) { setMEditId(null); return; }
                      setMEditId(m.id);
                      setMLoc(m.location); setMLabel(m.label); setMUrl(m.url);
                      setMParent(m.parent_id); setMTarget(m.target);
                      setMMinClass(m.min_class); setMSort(m.sort); setMEnabled(m.enabled);
                    }}>{mEditId === m.id ? t.cancelBtn : t.editBtn}</button>
                  <button className="ml-1 min-h-[28px] rounded-full border border-line px-3 font-bold text-danger"
                    onClick={() => guard(async () => {
                      await api.del(`/api/v1/admin/menu-items/${m.id}`);
                    }, t.deleted)}>{t.delBtn}</button>
                </td>
              </tr>
            );
          })}
          {(menuData?.items ?? []).length === 0 && (
            <tr><td colSpan={7} className="py-4 text-center text-sub">
              {menuData === null ? t.loadFail : t.empty}
            </td></tr>
          )}
        </tbody>
      </table>

      <div className="mt-4 flex flex-wrap items-end gap-2">
        <h3 className="w-full text-sm font-bold">
          {mEditId === null ? t.formTitleNew : fmt(t.formTitleEdit, { id: mEditId })}
        </h3>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldLoc}</span>
          <select value={mLoc} onChange={(e) => setMLoc(e.target.value)}
            className={INPUT_CLOUD}>
            <option value="topbar">{t.locTopbar}</option>
            <option value="sidebar">{t.locSidebar}</option>
            <option value="footer">{t.locFooter}</option>
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldLabel}</span>
          <input value={mLabel} onChange={(e) => setMLabel(e.target.value)} maxLength={50}
            className={INPUT_CLOUD} />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldUrl}</span>
          <input value={mUrl} onChange={(e) => setMUrl(e.target.value)} maxLength={300} placeholder={t.urlPh}
            className="min-h-[40px] w-64 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldParent}</span>
          <select value={mParent} onChange={(e) => setMParent(Number(e.target.value))}
            className={INPUT_CLOUD}>
            <option value={0}>{t.parentRoot}</option>
            {(menuData?.items ?? []).filter((x) => x.location === mLoc && x.parent_id === 0 && x.id !== mEditId).map((x) => (
              <option key={x.id} value={x.id}>{x.label}</option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldTarget}</span>
          <select value={mTarget} onChange={(e) => setMTarget(e.target.value)}
            className={INPUT_CLOUD}>
            <option value="_self">{t.targetSelf}</option>
            <option value="_blank">{t.targetBlank}</option>
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldMinClass}</span>
          <input type="number" min={0} max={99} value={mMinClass} onChange={(e) => setMMinClass(Number(e.target.value))}
            className="min-h-[40px] w-20 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">{t.fldSort}</span>
          <input type="number" value={mSort} onChange={(e) => setMSort(Number(e.target.value))}
            className="min-h-[40px] w-20 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky" />
        </label>
        <label className="flex items-center gap-2 pb-2 text-sm">
          <input type="checkbox" checked={mEnabled} onChange={(e) => setMEnabled(e.target.checked)} />
          {t.fldEnabled}
        </label>
        <button
          disabled={busy}
          className={BTN_MD_SKY}
          onClick={() => guard(async () => {
            const payload = {
              location: mLoc, label: mLabel.trim(), url: mUrl.trim(),
              parent_id: mParent, target: mTarget, min_class: mMinClass,
              sort: mSort, enabled: mEnabled,
            };
            if (mEditId === null) {
              await api.post("/api/v1/admin/menu-items", payload);
            } else {
              await api.put(`/api/v1/admin/menu-items/${mEditId}`, payload);
              setMEditId(null);
            }
            setMLabel(""); setMUrl(""); setMParent(0); setMSort(0);
          }, mEditId === null ? t.created : t.saved)}
        >{mEditId === null ? t.createBtn : t.saveBtn}</button>
      </div>
    </section>
  );
}
