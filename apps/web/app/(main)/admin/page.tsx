"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { ContentManage } from "@/components/content-manage";
import { StaffTools } from "@/components/staff-tools";

interface Overview {
  pending_reviews: number;
  open_reports: number;
  users: number;
  torrents: number;
  banned_users: number;
}

interface PendingTorrent {
  id: number;
  name: string;
  owner_id: number | null;
  size: number;
  created_at: string;
}

interface Report {
  id: number;
  reporter_id: number;
  ref_type: string;
  ref_id: number;
  reason: string;
  created_at: string;
}

interface AdminUser {
  id: number;
  username: string;
  email: string;
  class_id: number;
  status: number;
  created_at: string;
}

interface AuditRow {
  id: number;
  actor_id: number | null;
  action: string;
  created_at: string;
}

interface PanelEntry {
  panel: string;
  name: string;
  url: string;
  info: string;
}

interface CheaterRow {
  user_id: number;
  username: string;
  torrent_id: number | null;
  name: string | null;
  upspeed: number;
  uploaded_delta: number;
  announced_at: string;
}

interface SiteSetting {
  name: string;
  value: string;
  updated_at: string;
}

type AdminTab =
  | "overview"
  | "panel"
  | "settings"
  | "reviews"
  | "reports"
  | "users"
  | "audit"
  | "cheaters"
  | "content"
  | "tools";

/** 管理组面板（staffpanel.php 复刻）+ 站点设定 + 管理系统（审核/举报/用户/审计） */
export default function AdminPage({
  searchParams,
}: {
  searchParams: Promise<{ tool?: string }>;
}) {
  const { dict, locale } = useI18n();
  const a = dict.admin;
  const [tab, setTab] = useState<AdminTab>(() => {
    if (typeof window === "undefined") return "panel";
    const tool = new URLSearchParams(window.location.search).get("tool");
    if (tool === "cheaters") return "cheaters";
    if (["faqmanage", "modrules", "catmanage", "bans", "massmail"].includes(tool ?? "")) return "tools";
    return "panel";
  });
  const [cheaters, setCheaters] = useState<CheaterRow[]>([]);
  const [ov, setOv] = useState<Overview | null>(null);
  const [reviews, setReviews] = useState<PendingTorrent[]>([]);
  const [reports, setReports] = useState<Report[]>([]);
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [audit, setAudit] = useState<AuditRow[]>([]);
  const [panel, setPanel] = useState<PanelEntry[]>([]);
  const [role, setRole] = useState("");
  const [settings, setSettings] = useState<SiteSetting[]>([]);
  const [settingsEditable, setSettingsEditable] = useState(false);
  const [editing, setEditing] = useState<Record<string, string>>({});
  const [userQ, setUserQ] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  const loadCheaters = useCallback(async () => {
    try {
      const r = await api.get<CheaterRow[]>("/api/v1/admin/cheaters");
      setCheaters(r);
    } catch {
      setCheaters([]);
    }
  }, []);

  const load = useCallback(async () => {
    try {
      const [ovr, rev, rep, aud, pnl] = await Promise.all([
        api.get<Overview>("/api/v1/admin/overview"),
        api.get<PendingTorrent[]>("/api/v1/admin/reviews"),
        api.get<Report[]>("/api/v1/admin/reports"),
        api.get<AuditRow[]>("/api/v1/admin/audit"),
        api.get<{ entries: PanelEntry[]; role: string }>("/api/v1/admin/staffpanel"),
      ]);
      setOv(ovr);
      setReviews(rev);
      setReports(rep);
      setAudit(aud);
      setPanel(pnl.entries);
      setRole(pnl.role);
      if (pnl.role === "sysop" || pnl.role === "administrator") {
        const st = await api.get<{ settings: SiteSetting[]; editable: boolean }>(
          "/api/v1/admin/settings",
        );
        setSettings(st.settings);
        setSettingsEditable(st.editable);
      }
      setTab("panel");
    } catch (e) {
      setMsg(e instanceof ApiError && e.code === 2003 ? a.needAdmin : dict.common.loadFailed);
    }
  }, [a, dict]);

  useEffect(() => {
    load();
  }, [load]);

  async function searchUsers() {
    try {
      setUsers(
        await api.get<AdminUser[]>(`/api/v1/admin/users?q=${encodeURIComponent(userQ)}`),
      );
    } catch {
      setMsg(a.searchUsersFail);
    }
  }

  async function decide(torrentId: number, approve: boolean) {
    const reason = approve ? "" : (prompt(a.rejectReason) ?? "");
    if (!approve && !reason) return;
    try {
      await api.post("/api/v1/admin/reviews/decide", {
        torrent_id: torrentId,
        approve,
        reason,
      });
      setMsg(approve ? fmt(a.approved, { id: torrentId }) : fmt(a.rejected, { id: torrentId }));
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    }
  }

  async function resolveReport(id: number) {
    try {
      await api.post("/api/v1/admin/reports/resolve", { report_id: id });
      setMsg(fmt(a.resolved, { id }));
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    }
  }

  async function setUserStatus(userId: number, status: number) {
    try {
      await api.post("/api/v1/admin/users/status", { user_id: userId, status });
      setMsg(fmt(a.statusChanged, { id: userId, status: a.status[status] }));
      searchUsers();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    }
  }

  async function setUserClass(userId: number, classId: number) {
    try {
      await api.post("/api/v1/admin/users/class", { user_id: userId, class_id: classId });
      setMsg(fmt(a.classChanged, { id: userId, cls: classId }));
      searchUsers();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    }
  }

  async function saveSetting(name: string) {
    const value = editing[name];
    if (value === undefined) return;
    try {
      await api.put("/api/v1/admin/settings", { name, value });
      setMsg(fmt(a.settingSaved, { name }));
      const st = await api.get<{ settings: SiteSetting[]; editable: boolean }>(
        "/api/v1/admin/settings",
      );
      setSettings(st.settings);
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    }
  }

  const TABS = [
    ["panel", a.tabs.panel],
    ["overview", a.tabs.overview],
    ...(role === "sysop" || role === "administrator"
      ? ([["settings", a.tabs.settings]] as const)
      : []),
    ["reviews", fmt(a.tabs.reviews, { n: reviews.length })],
    ["reports", fmt(a.tabs.reports, { n: reports.length })],
    ["users", a.tabs.users],
    ["audit", a.tabs.audit],
    ["cheaters", a.cheatersTitle],
    ["content", a.tabs.content],
    ["tools", dict.stafftools.title],
  ] as const;

  const panelGroups: [string, PanelEntry[]][] = ["sysop", "admin", "moderator"]
    .map((g) => [g, panel.filter((e) => e.panel === g)] as [string, PanelEntry[]])
    .filter(([, entries]) => entries.length > 0);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{a.panelTitle}</h1>

      <div className="flex flex-wrap gap-2" role="tablist">
        {TABS.map(([key, label]) => (
          <button
            key={key}
            role="tab"
            aria-selected={tab === key}
            onClick={() => setTab(key)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${
              tab === key ? "bg-sky text-white" : "border border-line bg-white text-sub"
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>
      )}

      {/* 管理组面板：三组 colhead 表格（SysOp/Administrator/Moderator） */}
      {tab === "panel" &&
        panelGroups.map(([group, entries]) => (
          <section key={group} className="nexus-detail">
            <h2 className="mb-2 text-center text-base font-bold text-ink">
              ..:: {a.groups[group]} ::..
            </h2>
            <table className="nexus-table">
              <thead>
                <tr>
                  <td className="colhead">{a.colOptionName}</td>
                  <td className="colhead">{a.colInfo}</td>
                </tr>
              </thead>
              <tbody>
                {entries.map((e) => (
                  <tr key={`${e.panel}-${e.name}`}>
                    <td className="rowfollow font-bold">
                      <a href={e.url}>{e.name}</a>
                    </td>
                    <td className="rowfollow">{e.info}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ))}
      {tab === "panel" && panelGroups.length === 0 && (
        <p className="py-6 text-center text-sub">{a.panelEmpty}</p>
      )}

      {tab === "overview" && ov && (
        <section className="grid grid-cols-2 gap-3 md:grid-cols-5">
          {[
            [a.pendingReviews, ov.pending_reviews],
            [a.openReports, ov.open_reports],
            [a.users, ov.users],
            [a.torrents, ov.torrents],
            [a.bannedUsers, ov.banned_users],
          ].map(([label, v]) => (
            <div
              key={String(label)}
              className="rounded-[var(--r-md)] border border-line bg-white p-4 text-center shadow-[var(--shadow-card)]"
            >
              <p className="text-xs text-sub">{label}</p>
              <p className="num mt-1 text-2xl text-sky">{String(v)}</p>
            </div>
          ))}
        </section>
      )}

      {/* 站点设定：键值表（sysop 可编辑） */}
      {tab === "settings" && (
        <section className="nexus-detail">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">{a.colSettingName}</td>
                <td className="colhead">{a.colSettingValue}</td>
                <td className="colhead">{a.colSettingUpdated}</td>
                {settingsEditable && <td className="colhead">{a.colSettingAction}</td>}
              </tr>
            </thead>
            <tbody>
              {settings.map((s) => (
                <tr key={s.name}>
                  <td className="rowfollow font-mono text-xs">{s.name}</td>
                  <td className="rowfollow">
                    {settingsEditable ? (
                      <input
                        className="uc-input-wide"
                        defaultValue={s.value}
                        onChange={(e) =>
                          setEditing((prev) => ({ ...prev, [s.name]: e.target.value }))
                        }
                      />
                    ) : (
                      s.value
                    )}
                  </td>
                  <td className="rowfollow text-xs text-sub">
                    {new Date(s.updated_at).toLocaleString(dateLocale(locale))}
                  </td>
                  {settingsEditable && (
                    <td className="rowfollow">
                      <button
                        onClick={() => saveSetting(s.name)}
                        className="min-h-[32px] rounded-full bg-sky px-3 text-xs font-bold text-white"
                      >
                        {a.saveSetting}
                      </button>
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {tab === "reviews" && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <ul className="flex flex-col divide-y divide-line">
            {reviews.map((t) => (
              <li key={t.id} className="flex items-center gap-3 py-2">
                <div className="flex-1">
                  <p className="text-sm font-bold">{t.name}</p>
                  <p className="text-xs text-sub">
                    #{t.id} · {fmt(a.uploader, { name: t.owner_id ?? dict.torrent.anonymous })} ·{" "}
                    {(t.size / 1024 / 1024 / 1024).toFixed(2)}GB
                  </p>
                </div>
                <button
                  onClick={() => decide(t.id, true)}
                  className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white"
                >
                  {a.approve}
                </button>
                <button
                  onClick={() => decide(t.id, false)}
                  className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white"
                >
                  {a.reject}
                </button>
              </li>
            ))}
            {reviews.length === 0 && <li className="py-6 text-center text-sub">{a.queueEmpty}</li>}
          </ul>
        </section>
      )}

      {tab === "reports" && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <ul className="flex flex-col divide-y divide-line">
            {reports.map((r) => (
              <li key={r.id} className="flex items-center gap-3 py-2">
                <div className="flex-1">
                  <p className="text-sm">
                    <span className="rounded-full bg-sun/30 px-2 py-0.5 text-[10px]">
                      {r.ref_type}
                    </span>{" "}
                    #{r.ref_id}
                  </p>
                  <p className="text-xs text-sub">
                    {r.reason} · {fmt(a.reporter, { id: r.reporter_id })}
                  </p>
                </div>
                <button
                  onClick={() => resolveReport(r.id)}
                  className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                >
                  {a.resolve}
                </button>
              </li>
            ))}
            {reports.length === 0 && <li className="py-6 text-center text-sub">{a.noReports}</li>}
          </ul>
        </section>
      )}

      {tab === "users" && (
        <section className="flex flex-col gap-3">
          <div className="flex gap-2">
            <input
              value={userQ}
              onChange={(e) => setUserQ(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && searchUsers()}
              placeholder={a.searchPlaceholder}
              className="min-h-[44px] flex-1 rounded-[var(--r-sm)] border border-line px-3"
            />
            <button
              onClick={searchUsers}
              className="min-h-[44px] rounded-full bg-sky px-5 text-sm font-bold text-white"
            >
              {a.search}
            </button>
          </div>
          <ul className="flex flex-col divide-y divide-line rounded-[var(--r-lg)] border border-line bg-white p-4">
            {users.map((u) => (
              <li key={u.id} className="flex items-center gap-3 py-2">
                <div className="flex-1">
                  <p className="text-sm font-bold">
                    {u.username}
                    {u.status > 0 && (
                      <span className="ml-1 rounded-full bg-coral/20 px-2 py-0.5 text-[10px] text-danger">
                        {a.status[u.status]}
                      </span>
                    )}
                  </p>
                  <p className="text-xs text-sub">
                    #{u.id} · LV{u.class_id} · {u.email}
                  </p>
                </div>
                <select
                  value={u.class_id}
                  onChange={(e) => setUserClass(u.id, Number(e.target.value))}
                  className="min-h-[36px] rounded-full border border-line bg-white px-2 text-xs font-bold"
                  title={a.classAdjust}
                >
                  {a.classList.map(([id, label]) => (
                    <option key={id} value={id}>
                      {label}
                    </option>
                  ))}
                </select>
                <button
                  onClick={() => setUserStatus(u.id, u.status >= 2 ? 0 : 2)}
                  className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${
                    u.status >= 2 ? "bg-mint text-white" : "border border-line text-danger"
                  }`}
                >
                  {u.status >= 2 ? a.unban : a.ban}
                </button>
              </li>
            ))}
            {users.length === 0 && (
              <li className="py-6 text-center text-sub">{a.searchFirst}</li>
            )}
          </ul>
        </section>
      )}

      {tab === "content" && (
        <section className="nexus-detail">
          <h2 className="mb-3 text-base font-bold text-ink">{a.tabs.content}</h2>
          <ContentManage />
        </section>
      )}

      {tab === "tools" && <StaffTools />}

      {tab === "cheaters" && (
        <section className="nexus-detail">
          <h2 className="mb-2 text-base font-bold text-ink">{a.cheatersTitle}</h2>
          <p className="mb-2 text-xs text-sub">{a.cheatersNote}</p>
          <button
            onClick={loadCheaters}
            className="mb-3 min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white"
          >
            {a.cheatersScan}
          </button>
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table">
              <tbody>
                <tr>
                  <td className="colhead">{a.cheaterUser}</td>
                  <td className="colhead">{a.cheaterTorrent}</td>
                  <td className="colhead">{a.cheaterSpeed}</td>
                  <td className="colhead">{a.cheaterDelta}</td>
                  <td className="colhead">{a.cheaterAt}</td>
                </tr>
                {cheaters.map((row) => (
                  <tr key={`${row.user_id}-${row.torrent_id}`}>
                    <td className="rowfollow">
                      {row.username} #{row.user_id}
                    </td>
                    <td className="rowfollow">
                      {row.torrent_id ? (
                        <a href={`/torrent/${row.torrent_id}`}>{row.name ?? `#${row.torrent_id}`}</a>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td className="rowfollow num">
                      {(row.upspeed / 1024 / 1024).toFixed(1)} MB/s
                    </td>
                    <td className="rowfollow num">{(row.uploaded_delta / 1024 ** 3).toFixed(2)} GB</td>
                    <td className="rowfollow text-xs text-sub">
                      {new Date(row.announced_at).toLocaleString(dateLocale(locale))}
                    </td>
                  </tr>
                ))}
                {cheaters.length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-6 text-center text-sub">
                      {a.cheatersEmpty}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </section>
      )}

      {tab === "audit" && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <ul className="flex flex-col divide-y divide-line text-sm">
            {audit.map((row) => (
              <li key={row.id} className="flex items-center justify-between py-2">
                <span className="font-mono text-xs">{row.action}</span>
                <span className="text-xs text-sub">
                  {fmt(a.actor, { id: row.actor_id ?? "-" })} ·{" "}
                  {new Date(row.created_at).toLocaleString(dateLocale(locale))}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
