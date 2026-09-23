"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 置顶促销面板（从 admin-p2-tools.tsx 按域拆出，300 门禁）：
 *  首页公告条的增删与启停。 */

interface StickyPromo {
  id: number;
  title: string;
  url: string | null;
  badge: string | null;
  starts_at: string;
  ends_at: string;
  enabled: boolean;
  sort: number;
}

export function StickyPromos({ flash }: { flash: (m: string) => void }) {
  const { dict, locale } = useI18n();
  const at = dict.adminPromos;
  const [rows, setRows] = useState<StickyPromo[]>([]);
  const [edit, setEdit] = useState<{
    title: string;
    url: string;
    badge: string;
  }>({
    title: "",
    url: "",
    badge: "",
  });
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.get("/api/v1/admin/sticky-promos"));
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const save = async () => {
    if (!edit.title.trim()) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/sticky-promos", {
        title: edit.title,
        url: edit.url || undefined,
        badge: edit.badge || undefined,
      });
      flash(at.added);
      setEdit({ title: "", url: "", badge: "" });
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
          {at.fTitle}
          <input
            value={edit.title}
            onChange={(e) => setEdit({ ...edit, title: e.target.value })}
          />
        </label>
        <label>
          {at.fUrl}
          <input
            value={edit.url}
            onChange={(e) => setEdit({ ...edit, url: e.target.value })}
            placeholder="/torrents?official=1"
          />
        </label>
        <label>
          {at.fBadge}
          <input
            value={edit.badge}
            onChange={(e) => setEdit({ ...edit, badge: e.target.value })}
            placeholder={at.badgePh}
          />
        </label>
        <button
          className="baozi-button"
          disabled={busy || !edit.title.trim()}
          onClick={save}
        >
          {at.save7}
        </button>
      </section>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{at.thId}</td>
            <td className="colhead">{at.thTitle}</td>
            <td className="colhead">{at.thBadge}</td>
            <td className="colhead">{at.thRange}</td>
            <td className="colhead">{at.thEnabled}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>
                {r.url ? (
                  <a className="text-link" href={r.url}>
                    {r.title}
                  </a>
                ) : (
                  r.title
                )}
              </td>
              <td>{r.badge ?? "—"}</td>
              <td className="text-xs">
                {new Date(r.starts_at).toLocaleDateString(
                  dateLocale(locale),
                )}{" "}
                ~{" "}
                {new Date(r.ends_at).toLocaleDateString(dateLocale(locale))}
              </td>
              <td>{r.enabled ? at.yes : at.no}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={async () => {
                    try {
                      await api.put(`/api/v1/admin/sticky-promos/${r.id}`, {
                        title: r.title,
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
                      await api.del(`/api/v1/admin/sticky-promos/${r.id}`);
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
