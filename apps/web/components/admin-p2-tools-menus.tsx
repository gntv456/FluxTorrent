"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 自定义菜单面板（从 admin-p2-tools.tsx 按域拆出，300 门禁）：
 *  侧栏 / 页脚 / 顶栏菜单项的增删与启停。 */

interface MenuItem {
  id: number;
  location: string;
  label: string;
  url: string;
  sort: number;
  enabled: boolean;
}

const LOCATIONS = ["sidebar", "footer", "topbar"];

export function MenuItems({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const at = dict.adminMenus;
  const [rows, setRows] = useState<MenuItem[]>([]);
  const [edit, setEdit] = useState<{
    location: string;
    label: string;
    url: string;
  }>({
    location: "sidebar",
    label: "",
    url: "",
  });
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      // api.get 返回 data：menu-items 是 { custom_enabled, items }（修复：此前
      // 直接 setRows(对象) 导致 rows.map 崩，i.map is not a function）
      const r = await api.get<{ items?: MenuItem[] } | MenuItem[]>(
        "/api/v1/admin/menu-items",
      );
      setRows(Array.isArray(r) ? r : (r.items ?? []));
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const save = async () => {
    if (!edit.label.trim() || !edit.url.trim()) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/menu-items", edit);
      flash(at.added);
      setEdit({ location: edit.location, label: "", url: "" });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold text-ink">{at.newTitle}</h2>
        <label>
          {at.fLoc}
          <select
            value={edit.location}
            onChange={(e) => setEdit({ ...edit, location: e.target.value })}
          >
            {LOCATIONS.map((v) => (
              <option key={v} value={v}>
                {at.locLabels[v] ?? v}
              </option>
            ))}
          </select>
        </label>
        <label>
          {at.fName}
          <input
            value={edit.label}
            onChange={(e) => setEdit({ ...edit, label: e.target.value })}
          />
        </label>
        <label>
          {at.fUrl}
          <input
            value={edit.url}
            onChange={(e) => setEdit({ ...edit, url: e.target.value })}
            placeholder="https://..."
          />
        </label>
        <button
          className="baozi-button"
          disabled={busy || !edit.label.trim() || !edit.url.trim()}
          onClick={save}
        >
          {at.save}
        </button>
      </section>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{at.thId}</td>
            <td className="colhead">{at.thLoc}</td>
            <td className="colhead">{at.thName}</td>
            <td className="colhead">{at.thUrl}</td>
            <td className="colhead">{at.thSort}</td>
            <td className="colhead">{at.thEnabled}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>{at.locLabels[r.location] ?? r.location}</td>
              <td>{r.label}</td>
              <td className="max-w-[200px] truncate text-xs">{r.url}</td>
              <td>{r.sort}</td>
              <td>{r.enabled ? at.yes : at.no}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={async () => {
                    try {
                      await api.put(`/api/v1/admin/menu-items/${r.id}`, {
                        enabled: !r.enabled,
                      });
                      flash(r.enabled ? at.untoggled : at.toggled);
                      load();
                    } catch {
                      flash(at.opFail);
                    }
                  }}
                >
                  {r.enabled ? at.disable : at.enable}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/menu-items/${r.id}`);
                      flash(at.deleted);
                      load();
                    } catch {
                      flash(at.delFail);
                    }
                  }}
                >
                  {at.del}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
