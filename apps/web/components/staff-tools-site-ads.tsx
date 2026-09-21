"use client";

import { api, ApiError } from "@/lib/api-client";
import { BTN_SM_BOLD as PLAIN_BTN_CLS } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import type { AdItem } from "./staff-tools-site-shared";

/** 站点工具·广告管理 tab（从 staff-tools-site.tsx 按域拆出）。 */
export interface AdEdit {
  id: number | null;
  title: string;
  html: string;
  position: string;
}

export function SiteAdsTab(props: {
  ads: AdItem[];
  setAds: React.Dispatch<React.SetStateAction<AdItem[]>>;
  adEdit: AdEdit;
  setAdEdit: React.Dispatch<React.SetStateAction<AdEdit>>;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
  load: () => void;
}) {
  const { ads, setAds, adEdit, setAdEdit, busy, guard, load } = props;
  const { dict } = useI18n();
  const t = dict.stafftools;
  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">
          {adEdit.id === null ? t.adsNew : t.adsEdit}
        </h2>
        <div className="cmgmt-form">
          <label>
            {dict.cmgmt.fldTitle}
            <input
              value={adEdit.title}
              onChange={(e) => setAdEdit({ ...adEdit, title: e.target.value })}
            />
          </label>
          <label>
            {t.adsHtml}
            <textarea
              rows={3}
              value={adEdit.html}
              onChange={(e) => setAdEdit({ ...adEdit, html: e.target.value })}
            />
          </label>
          <label>
            {t.adsPosition}
            <select
              value={adEdit.position}
              onChange={(e) =>
                setAdEdit({ ...adEdit, position: e.target.value })
              }
            >
              <option value="header">Header</option>
              <option value="footer">Footer</option>
              <option value="sidebar">Sidebar</option>
            </select>
          </label>
          <div className="flex gap-2">
            <button
              className="baozi-button"
              disabled={busy || !adEdit.title.trim() || !adEdit.html.trim()}
              onClick={() =>
                guard(async () => {
                  if (adEdit.id === null)
                    await api.post("/api/v1/admin/ads", {
                      title: adEdit.title,
                      html: adEdit.html,
                      position: adEdit.position,
                    });
                  else
                    await api.put(`/api/v1/admin/ads/${adEdit.id}`, {
                      title: adEdit.title,
                      html: adEdit.html,
                      position: adEdit.position,
                    });
                  setAdEdit({
                    id: null,
                    title: "",
                    html: "",
                    position: "header",
                  });
                }, t.saved)
              }
            >
              {t.btnSave}
            </button>
            {adEdit.id !== null && (
              <button
                className={PLAIN_BTN_CLS}
                onClick={() =>
                  setAdEdit({
                    id: null,
                    title: "",
                    html: "",
                    position: "header",
                  })
                }
              >
                {dict.cmgmt.btnCancel}
              </button>
            )}
          </div>
        </div>
      </section>
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{dict.cmgmt.fldTitle}</td>
            <td className="colhead">{t.adsPosition}</td>
            <td className="colhead">{t.adsEnabled}</td>
            <td className="colhead text-right">{dict.cmgmt.colActions}</td>
          </tr>
          {ads.map((a) => (
            <tr key={a.id}>
              <td>{a.title}</td>
              <td className="text-xs">{a.position}</td>
              <td>{a.enabled ? "✅" : "⛔"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setAdEdit({
                      id: a.id,
                      title: a.title,
                      html: a.html,
                      position: a.position,
                    })
                  }
                >
                  {dict.cmgmt.btnEdit}
                </button>
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    guard(async () => {
                      await api.put(`/api/v1/admin/ads/${a.id}/toggle`);
                    }, t.saved)
                  }
                >
                  {t.adsToggle}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={() =>
                    guard(async () => {
                      await api.del(`/api/v1/admin/ads/${a.id}`);
                    }, t.deleted)
                  }
                >
                  {dict.cmgmt.btnDelete}
                </button>
              </td>
            </tr>
          ))}
          {ads.length === 0 && (
            <tr>
              <td colSpan={4} className="py-6 text-center text-sub">
                {t.adsEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </>
  );
}
