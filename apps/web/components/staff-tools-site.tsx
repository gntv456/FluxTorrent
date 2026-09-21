"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";
import { StaffSiteLists } from "./staff-tools-site-lists";
import type {
  AdItem,
  AgentRow,
  CleanupResult,
  NotConnectRow,
  PollRow,
  SiteStats,
  UploaderRow,
} from "./staff-tools-site-shared";

/** 站点域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  统计（stats）/ 清理（cleanup）/ 广告（ads）/ 插件（plugins）。
 *  只读列表（notconnect/uploaders/agents/polls）拆至
 *  ./staff-tools-site-lists.tsx；类型拆至 ./staff-tools-site-shared.ts。 */

/** 圆角描边小按钮 */
const PLAIN_BTN_CLS =
  BTN_SM_BOLD;
/** Redis 状态文案配色 */
function redisCls(v: string) {
  return `text-lg font-bold ${v === "up" ? "text-success" : "text-danger"}`;
}

export function StaffSitePanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [stats, setStats] = useState<SiteStats | null>(null);
  const [cleanupResult, setCleanupResult] = useState<CleanupResult | null>(
    null,
  );
  const [ads, setAds] = useState<AdItem[]>([]);
  const [notConnectRows, setNotConnectRows] = useState<NotConnectRow[]>([]);
  const [uploaderRows, setUploaderRows] = useState<UploaderRow[]>([]);
  const [agentRows, setAgentRows] = useState<AgentRow[]>([]);
  const [pollRows, setPollRows] = useState<PollRow[]>([]);
  const [pluginList, setPluginList] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [adEdit, setAdEdit] = useState<{
    id: number | null;
    title: string;
    html: string;
    position: string;
  }>({ id: null, title: "", html: "", position: "header" });

  const load = useCallback(async () => {
    api
      .get<SiteStats | null>("/api/v1/admin/stats")
      .then(setStats)
      .catch(() => setStats(null));
    api
      .get<AdItem[]>("/api/v1/admin/ads")
      .then(setAds)
      .catch(() => setAds([]));
    api
      .get<NotConnectRow[]>("/api/v1/admin/notconnectable")
      .then(setNotConnectRows)
      .catch(() => setNotConnectRows([]));
    api
      .get<UploaderRow[]>("/api/v1/admin/uploaders")
      .then(setUploaderRows)
      .catch(() => setUploaderRows([]));
    api
      .get<AgentRow[]>("/api/v1/admin/allagents")
      .then(setAgentRows)
      .catch(() => setAgentRows([]));
    api
      .get<PollRow[]>("/api/v1/admin/polloverview")
      .then(setPollRows)
      .catch(() => setPollRows([]));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      {/* 统计（stats） */}
      {tab === "stats" && stats && (
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          {(
            [
              [t.stUsers, stats.users],
              [t.stTorrents, stats.torrents],
              [t.stSeeding, stats.seeding],
              [t.stLeeching, stats.leeching],
              [t.stComments, stats.comments],
              [t.stMessages, stats.messages],
            ] as [string, number][]
          ).map(([label, v]) => (
            <div key={label} className="baozi-panel p-4">
              <p className="text-xs text-sub">{label}</p>
              <p className="num text-2xl font-bold text-ink">
                {v.toLocaleString("zh-CN")}
              </p>
            </div>
          ))}
          <div className="baozi-panel p-4">
            <p className="text-xs text-sub">{t.stRedis}</p>
            <p className={redisCls(stats.redis)}>
              {stats.redis === "up" ? "✅ up" : "⛔ down"}
            </p>
          </div>
          <div className="baozi-panel p-4">
            <p className="text-xs text-sub">{t.stUptime}</p>
            <p className="num text-lg font-bold text-ink">
              {Math.floor(stats.uptime_secs / 3600)}h{" "}
              {Math.floor((stats.uptime_secs % 3600) / 60)}m
            </p>
          </div>
        </div>
      )}

      {/* 清除缓存 + 做清理（clearcache/docleanup） */}
      {tab === "cleanup" && (
        <>
          <section className="baozi-panel flex flex-col gap-3 p-4">
            <h2 className="text-base font-bold text-ink">{t.ccTitle}</h2>
            <p className="text-xs text-sub">{t.ccNote}</p>
            <button
              className="baozi-button self-start"
              disabled={busy}
              onClick={() =>
                guard(async () => {
                  const r = await api.post<{ cleared: number }>(
                    "/api/v1/admin/clearcache",
                  );
                  flash(t.ccDone.replace("{n}", String(r.cleared)));
                }, "")
              }
            >
              {t.ccBtn}
            </button>
          </section>
          <section className="baozi-panel flex flex-col gap-3 p-4">
            <h2 className="text-base font-bold text-ink">{t.dcuTitle}</h2>
            <p className="text-xs text-sub">{t.dcuNote}</p>
            <button
              className="baozi-button self-start"
              disabled={busy}
              onClick={() =>
                guard(async () => {
                  const r = await api.post<CleanupResult>(
                    "/api/v1/admin/docleanup",
                  );
                  setCleanupResult(r);
                }, t.dcuDone)
              }
            >
              {t.dcuBtn}
            </button>
            {cleanupResult && (
              <table className="nexus-table">
                <tbody>
                  <tr>
                    <td className="rowhead">{t.dcuPromos}</td>
                    <td className="rowfollow num">
                      {cleanupResult.expired_promotions}
                    </td>
                  </tr>
                  <tr>
                    <td className="rowhead">{t.dcuWarns}</td>
                    <td className="rowfollow num">
                      {cleanupResult.expired_warnings}
                    </td>
                  </tr>
                  <tr>
                    <td className="rowhead">{t.dcuLogins}</td>
                    <td className="rowfollow num">
                      {cleanupResult.old_login_events}
                    </td>
                  </tr>
                  <tr>
                    <td className="rowhead">{t.dcuResets}</td>
                    <td className="rowfollow num">
                      {cleanupResult.old_password_resets}
                    </td>
                  </tr>
                </tbody>
              </table>
            )}
          </section>
        </>
      )}

      {/* 广告管理（admanage） */}
      {tab === "ads" && (
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
                  onChange={(e) =>
                    setAdEdit({ ...adEdit, title: e.target.value })
                  }
                />
              </label>
              <label>
                {t.adsHtml}
                <textarea
                  rows={3}
                  value={adEdit.html}
                  onChange={(e) =>
                    setAdEdit({ ...adEdit, html: e.target.value })
                  }
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
      )}

      {/* 只读列表（拆至 ./staff-tools-site-lists.tsx） */}
      {(tab === "notconnect" ||
        tab === "uploaders" ||
        tab === "agents" ||
        tab === "polls") && (
        <StaffSiteLists
          tab={tab}
          notConnectRows={notConnectRows}
          uploaderRows={uploaderRows}
          agentRows={agentRows}
          pollRows={pollRows}
        />
      )}

      {/* 插件清单（M28 只读：启停由插件配置决定） */}
      {tab === "plugins" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">
            {t.pluginsTitle ?? "插件清单"}
          </h2>
          {pluginList === null ? (
            <button
              className="baozi-button"
              onClick={async () => {
                try {
                  const r = await api.get<{ plugins: string[] }>(
                    "/api/v1/admin/plugins",
                  );
                  setPluginList(r.plugins);
                } catch {
                  setPluginList([]);
                }
              }}
            >
              {t.pluginsLoad ?? "加载"}
            </button>
          ) : (
            <ul className="flex flex-col gap-1 text-sm">
              {pluginList.map((pl) => (
                <li key={pl} className="flex items-center gap-2">
                  <span className="fun-status fun-status--normal">on</span>
                  <code className="text-xs">{pl}</code>
                </li>
              ))}
              {pluginList.length === 0 && (
                <li className="text-sub">{t.pluginsEmpty ?? "无插件"}</li>
              )}
            </ul>
          )}
          <p className="mt-2 text-xs text-sub">
            {t.pluginsNote ?? "插件启停由服务端配置决定，此处为只读清单"}
          </p>
        </section>
      )}
    </>
  );
}
