"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 第八轮 P3-16：Tracker URL 管理（NexusPHP tracker_urls 口径）
 *  多 announce 地址：默认位 / 启用 / 优先级 */

interface TrackerUrlRow {
  id: number;
  url: string;
  is_default: boolean;
  enabled: boolean;
  priority: number;
  updated_at: string;
}

export function AdminTrackers() {
  const { dict, locale } = useI18n();
  const at = dict.adminTrackers;
  const [rows, setRows] = useState<TrackerUrlRow[]>([]);
  const [edit, setEdit] = useState<{
    id: number | null;
    f: { url: string; is_default: boolean; enabled: boolean; priority: string };
  }>({
    id: null,
    f: { url: "", is_default: false, enabled: true, priority: "0" },
  });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<TrackerUrlRow[]>("/api/v1/admin/tracker-urls"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.adminTrackers.loadFail);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    setBusy(true);
    try {
      const payload = {
        url: edit.f.url,
        is_default: edit.f.is_default,
        enabled: edit.f.enabled,
        priority: Number(edit.f.priority) || 0,
      };
      if (edit.id === null)
        await api.post("/api/v1/admin/tracker-urls", payload);
      else await api.put(`/api/v1/admin/tracker-urls/${edit.id}`, payload);
      flash(at.saved);
      setEdit({
        id: null,
        f: { url: "", is_default: false, enabled: true, priority: "0" },
      });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {edit.id === null
            ? at.formNew
            : fmt(at.formEdit, { id: edit.id })}
        </h2>
        <p className="mb-2 text-xs text-sub">{at.hint}</p>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-1 flex-col gap-1 text-xs">
            URL
            <input
              value={edit.f.url}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, url: e.target.value } })
              }
              placeholder="https://tracker.example.com/announce"
              className={inp}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {at.fPriority}
            <input
              type="number"
              value={edit.f.priority}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, priority: e.target.value } })
              }
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input
              type="checkbox"
              checked={edit.f.is_default}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, is_default: e.target.checked },
                })
              }
            />
            {at.isDefault}
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input
              type="checkbox"
              checked={edit.f.enabled}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, enabled: e.target.checked },
                })
              }
            />
            {at.enabled}
          </label>
          <button
            className="baozi-button"
            disabled={busy || !edit.f.url.trim()}
            onClick={save}
          >
            {at.save}
          </button>
          {edit.id !== null && (
            <button
              className={BTN_SM_BOLD}
              onClick={() =>
                setEdit({
                  id: null,
                  f: {
                    url: "",
                    is_default: false,
                    enabled: true,
                    priority: "0",
                  },
                })
              }
            >
              {at.cancel}
            </button>
          )}
        </div>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thId}</td>
            <td className="colhead">{at.thUrl}</td>
            <td className="colhead">{at.thDefault}</td>
            <td className="colhead">{at.thEnabled}</td>
            <td className="colhead">{at.thPriority}</td>
            <td className="colhead">{at.thUpdated}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td className="num">{r.id}</td>
              <td className="font-mono">{r.url}</td>
              <td className="text-center">{r.is_default ? "✅" : "—"}</td>
              <td>{r.enabled ? at.on : at.off}</td>
              <td className="num">{r.priority}</td>
              <td className="text-sub">
                {new Date(r.updated_at).toLocaleString(dateLocale(locale))}
              </td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEdit({
                      id: r.id,
                      f: {
                        url: r.url,
                        is_default: r.is_default,
                        enabled: r.enabled,
                        priority: String(r.priority),
                      },
                    })
                  }
                >
                  {at.edit}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/tracker-urls/${r.id}`);
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
              <td colSpan={7} className="py-6 text-center text-sub">
                {at.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
