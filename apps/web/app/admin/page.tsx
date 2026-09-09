"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

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

/** M29 管理后台（staff 专用）：概览 + 审核队列 + 举报 + 用户 + 审计 */
export default function AdminPage() {
  const { dict, locale } = useI18n();
  const [tab, setTab] = useState<"overview" | "reviews" | "reports" | "users" | "audit">("overview");
  const [ov, setOv] = useState<Overview | null>(null);
  const [reviews, setReviews] = useState<PendingTorrent[]>([]);
  const [reports, setReports] = useState<Report[]>([]);
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [audit, setAudit] = useState<AuditRow[]>([]);
  const [userQ, setUserQ] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/admin/overview"));
      setReviews(await api.get<PendingTorrent[]>("/api/v1/admin/reviews"));
      setReports(await api.get<Report[]>("/api/v1/admin/reports"));
      setAudit(await api.get<AuditRow[]>("/api/v1/admin/audit"));
    } catch (e) {
      setMsg(e instanceof ApiError && e.code === 2003 ? dict.admin.needAdmin : dict.common.loadFailed);
    }
  }, [dict]);

  useEffect(() => {
    load();
  }, [load]);

  async function searchUsers() {
    try {
      setUsers(
        await api.get<AdminUser[]>(`/api/v1/admin/users?q=${encodeURIComponent(userQ)}`),
      );
    } catch {
      setMsg(dict.admin.searchUsersFail);
    }
  }

  async function decide(torrentId: number, approve: boolean) {
    const reason = approve ? "" : (prompt(dict.admin.rejectReason) ?? "");
    if (!approve && !reason) return;
    try {
      await api.post("/api/v1/admin/reviews/decide", {
        torrent_id: torrentId,
        approve,
        reason,
      });
      setMsg(approve ? fmt(dict.admin.approved, { id: torrentId }) : fmt(dict.admin.rejected, { id: torrentId }));
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.admin.actionFailed);
    }
  }

  async function resolveReport(id: number) {
    try {
      await api.post("/api/v1/admin/reports/resolve", { report_id: id });
      setMsg(fmt(dict.admin.resolved, { id }));
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.admin.actionFailed);
    }
  }

  async function setUserStatus(userId: number, status: number) {
    try {
      await api.post("/api/v1/admin/users/status", { user_id: userId, status });
      setMsg(fmt(dict.admin.statusChanged, { id: userId, status: dict.admin.status[status] }));
      searchUsers();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.admin.actionFailed);
    }
  }

  const TABS = [
    ["overview", dict.admin.tabs.overview],
    ["reviews", fmt(dict.admin.tabs.reviews, { n: reviews.length })],
    ["reports", fmt(dict.admin.tabs.reports, { n: reports.length })],
    ["users", dict.admin.tabs.users],
    ["audit", dict.admin.tabs.audit],
  ] as const;

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.admin.title}</h1>

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

      {tab === "overview" && ov && (
        <section className="grid grid-cols-2 gap-3 md:grid-cols-5">
          {[
            [dict.admin.pendingReviews, ov.pending_reviews],
            [dict.admin.openReports, ov.open_reports],
            [dict.admin.users, ov.users],
            [dict.admin.torrents, ov.torrents],
            [dict.admin.bannedUsers, ov.banned_users],
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

      {tab === "reviews" && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <ul className="flex flex-col divide-y divide-line">
            {reviews.map((t) => (
              <li key={t.id} className="flex items-center gap-3 py-2">
                <div className="flex-1">
                  <p className="text-sm font-bold">{t.name}</p>
                  <p className="text-xs text-sub">
                    #{t.id} · {fmt(dict.admin.uploader, { name: t.owner_id ?? dict.torrent.anonymous })} ·{" "}
                    {(t.size / 1024 / 1024 / 1024).toFixed(2)}GB
                  </p>
                </div>
                <button
                  onClick={() => decide(t.id, true)}
                  className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white"
                >
                  {dict.admin.approve}
                </button>
                <button
                  onClick={() => decide(t.id, false)}
                  className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white"
                >
                  {dict.admin.reject}
                </button>
              </li>
            ))}
            {reviews.length === 0 && <li className="py-6 text-center text-sub">{dict.admin.queueEmpty}</li>}
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
                    {r.reason} · {fmt(dict.admin.reporter, { id: r.reporter_id })}
                  </p>
                </div>
                <button
                  onClick={() => resolveReport(r.id)}
                  className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                >
                  {dict.admin.resolve}
                </button>
              </li>
            ))}
            {reports.length === 0 && <li className="py-6 text-center text-sub">{dict.admin.noReports}</li>}
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
              placeholder={dict.admin.searchPlaceholder}
              className="min-h-[44px] flex-1 rounded-[var(--r-sm)] border border-line px-3"
            />
            <button
              onClick={searchUsers}
              className="min-h-[44px] rounded-full bg-sky px-5 text-sm font-bold text-white"
            >
              {dict.admin.search}
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
                        {dict.admin.status[u.status]}
                      </span>
                    )}
                  </p>
                  <p className="text-xs text-sub">
                    #{u.id} · LV{u.class_id} · {u.email}
                  </p>
                </div>
                <button
                  onClick={() => setUserStatus(u.id, u.status >= 2 ? 0 : 2)}
                  className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${
                    u.status >= 2 ? "bg-mint text-white" : "border border-line text-danger"
                  }`}
                >
                  {u.status >= 2 ? dict.admin.unban : dict.admin.ban}
                </button>
              </li>
            ))}
            {users.length === 0 && (
              <li className="py-6 text-center text-sub">{dict.admin.searchFirst}</li>
            )}
          </ul>
        </section>
      )}

      {tab === "audit" && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
          <ul className="flex flex-col divide-y divide-line text-sm">
            {audit.map((a) => (
              <li key={a.id} className="flex items-center justify-between py-2">
                <span className="font-mono text-xs">{a.action}</span>
                <span className="text-xs text-sub">
                  {fmt(dict.admin.actor, { id: a.actor_id ?? "-" })} ·{" "}
                  {new Date(a.created_at).toLocaleString(dateLocale(locale))}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
