"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { ContentManage } from "@/components/content-manage";
import { StaffTools } from "@/components/staff-tools";
import { AdminUsers } from "@/components/admin-users";
import { AdminTorrents } from "@/components/admin-torrents";
import { AdminP2Tools } from "@/components/admin-p2-tools";

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

interface AppealRow {
  id: number;
  username: string;
  kind: string;
  ref_id: number | null;
  body: string;
  status: string;
  result_note: string | null;
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

type AdminTab =
  | "overview"
  | "panel"
  | "reviews"
  | "reports"
  | "appeals"
  | "users"
  | "torrents"
  | "p2tools"
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
  const [appeals, setAppeals] = useState<AppealRow[]>([]);
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [audit, setAudit] = useState<AuditRow[]>([]);
  const [panel, setPanel] = useState<PanelEntry[]>([]);
  const [role, setRole] = useState("");
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
      const [ovr, rev, rep, aud, pnl, aps] = await Promise.all([
        api.get<Overview>("/api/v1/admin/overview"),
        api.get<PendingTorrent[]>("/api/v1/admin/reviews"),
        api.get<Report[]>("/api/v1/admin/reports"),
        api.get<AuditRow[]>("/api/v1/admin/audit"),
        api.get<{ entries: PanelEntry[]; role: string }>("/api/v1/admin/staffpanel"),
        api.get<AppealRow[]>("/api/v1/admin/appeals").catch(() => [] as AppealRow[]),
      ]);
      setOv(ovr);
      setReviews(rev);
      setReports(rep);
      setAudit(aud);
      setPanel(pnl.entries);
      setRole(pnl.role);
      setAppeals(aps);
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

  async function handleAppeal(id: number, accept: boolean) {
    const note = prompt(accept ? a.appealAcceptNote ?? "通过说明（可选）" : a.appealRejectNote ?? "驳回理由（必填）") ?? "";
    if (!accept && !note.trim()) return;
    try {
      await api.post("/api/v1/admin/appeals/handle", {
        appeal_id: id,
        accept,
        note,
      });
      setMsg(fmt(a.appealHandled, { id }));
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

  const TABS = [
    ["panel", a.tabs.panel],
    ["overview", a.tabs.overview],
    ["reviews", fmt(a.tabs.reviews, { n: reviews.length })],
    ["reports", fmt(a.tabs.reports, { n: reports.length })],
    ["appeals", a.tabs.appeals ?? "申诉"],
    ["users", a.tabs.users],
    ["torrents", a.tabs.torrents ?? "种子管理"],
    ["p2tools", a.tabs.p2tools ?? "运营配置"],
    ["audit", a.tabs.audit],
    ["cheaters", a.cheatersTitle],
    ["content", a.tabs.content],
    ["tools", dict.stafftools.title],
  ] as const;

  /** 12 个功能按职能分四组渲染（避免一条平铺的药丸带找不到功能） */
  const TAB_GROUPS: { label: string; keys: AdminTab[] }[] = [
    { label: a.groupOverview ?? "概览", keys: ["panel", "overview"] },
    {
      label: a.groupQueue ?? "处理队列",
      keys: ["reviews", "reports", "appeals", "cheaters"],
    },
    {
      label: a.groupManage ?? "用户与内容",
      keys: ["users", "torrents", "content", "audit"],
    },
    {
      label: a.groupOps ?? "运营与配置",
      keys: ["p2tools", "tools"],
    },
  ];

  const panelGroups: [string, PanelEntry[]][] = ["sysop", "admin", "moderator"]
    .map((g) => [g, panel.filter((e) => e.panel === g)] as [string, PanelEntry[]])
    .filter(([, entries]) => entries.length > 0);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{a.panelTitle}</h1>

      {/* 分组标签条：四组职能，组内药丸切换（不再一条平铺 12 个） */}
      <div className="flex flex-col gap-2" role="tablist">
        {TAB_GROUPS.map((group) => (
          <div key={group.label} className="flex flex-wrap items-center gap-2">
            <span className="min-w-[72px] shrink-0 text-xs font-bold text-sub">
              {group.label}
            </span>
            {group.keys.map((key) => {
              const item = TABS.find(([k]) => k === key);
              if (!item) return null;
              const [, label] = item;
              return (
                <button
                  key={key}
                  role="tab"
                  aria-selected={tab === key}
                  onClick={() => setTab(key)}
                  className={`min-h-[36px] rounded-full px-3.5 text-[13px] font-bold ${
                    tab === key
                      ? "bg-sky text-white"
                      : "border border-line bg-[var(--surface-card)] text-sub"
                  }`}
                >
                  {label}
                </button>
              );
            })}
          </div>
        ))}
      </div>

      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>
      )}

      {/* panel 页顶部：核心数字条（点击直达对应队列），不用再切"概览"才能看到 */}
      {tab === "panel" && ov && (
        <section className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-5">
          {[
            { label: a.pendingReviews, v: ov.pending_reviews, to: "reviews" as AdminTab },
            { label: a.openReports, v: ov.open_reports, to: "reports" as AdminTab },
            { label: a.users, v: ov.users, to: "users" as AdminTab },
            { label: a.torrents, v: ov.torrents, to: "torrents" as AdminTab },
            { label: a.bannedUsers, v: ov.banned_users, to: "users" as AdminTab },
          ].map((s) => (
            <button
              key={s.label}
              onClick={() => setTab(s.to)}
              title={a.overviewOpen ?? "点击打开对应管理页"}
              className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3 text-center shadow-[var(--shadow-card)] transition hover:border-sky"
            >
              <p className="text-xs text-sub">{s.label}</p>
              <p className="num mt-1 text-2xl text-sky">{s.v}</p>
            </button>
          ))}
        </section>
      )}

      {/* 站点设定入口：新版类型化设定页位于独立路由 /admin/settings，仅 sysop/administrator 可见 */}
      {tab === "panel" && (role === "sysop" || role === "administrator") && (
        <a
          href="/admin/settings"
          className="flex items-center justify-between gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)] transition hover:border-sky"
        >
          <span className="min-w-0">
            <span className="block font-bold text-ink">{dict.settingsAdmin.title}</span>
            <span className="mt-0.5 block text-xs text-sub">{dict.settingsAdmin.note}</span>
          </span>
          <span className="min-h-[44px] shrink-0 rounded-full bg-sky-deep px-5 text-sm font-bold leading-[44px] text-white">
            {dict.settingsAdmin.entryOpen}
          </span>
        </a>
      )}

      {/* 管理组面板：三组条目改为紧凑卡片网格（原为三张两列大表纵铺，密度过低） */}
      {tab === "panel" &&
        panelGroups.map(([group, entries]) => (
          <section key={group}>
            <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">
              {a.groups[group]}
            </h2>
            <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {entries.map((e) => (
                <a
                  key={`${e.panel}-${e.name}`}
                  href={e.url}
                  className="group rounded-[var(--r-md)] border border-line bg-[var(--surface-raised)] p-3 transition hover:border-sky"
                >
                  <span className="block text-sm font-bold text-ink group-hover:text-sky">
                    {e.name}
                  </span>
                  <span className="mt-0.5 block text-xs text-sub">{e.info}</span>
                </a>
              ))}
            </div>
          </section>
        ))}
      {tab === "panel" && panelGroups.length === 0 && (
        <p className="py-6 text-center text-sub">{a.panelEmpty}</p>
      )}

      {tab === "overview" && ov && (
        <section className="grid grid-cols-2 gap-3 md:grid-cols-5">
          {[
            { label: a.pendingReviews, v: ov.pending_reviews, to: "reviews" as AdminTab },
            { label: a.openReports, v: ov.open_reports, to: "reports" as AdminTab },
            { label: a.users, v: ov.users, to: "users" as AdminTab },
            { label: a.torrents, v: ov.torrents, to: "torrents" as AdminTab },
            { label: a.bannedUsers, v: ov.banned_users, to: "users" as AdminTab },
          ].map((s) => (
            <button
              key={s.label}
              onClick={() => setTab(s.to)}
              title={a.overviewOpen}
              className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 text-center shadow-[var(--shadow-card)] transition hover:border-sky"
            >
              <p className="text-xs text-sub">{s.label}</p>
              <p className="num mt-1 text-2xl text-sky">{s.v}</p>
            </button>
          ))}
        </section>
      )}

      {/* 站点设定已迁移至独立路由 /admin/settings（类型化控件 / 服务端校验 / 修改历史 / 导出导入） */}

      {tab === "reviews" && (
        <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
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
        <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
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

      {tab === "appeals" && (
        <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
          <ul className="flex flex-col divide-y divide-line">
            {appeals.map((ap) => (
              <li key={ap.id} className="flex items-center gap-3 py-2">
                <div className="flex-1">
                  <p className="text-sm">
                    <span className="rounded-full bg-sun/30 px-2 py-0.5 text-[10px]">
                      {ap.kind}
                    </span>{" "}
                    <b>{ap.username}</b>
                    {ap.ref_id !== null && <span className="text-xs text-sub"> · #{ap.ref_id}</span>}
                    {ap.status !== "open" && (
                      <span
                        className={`ml-1 rounded-full px-2 py-0.5 text-[10px] ${
                          ap.status === "accepted" ? "bg-mint/30" : "bg-coral/20 text-danger"
                        }`}
                      >
                        {ap.status === "accepted" ? (a.appealAccepted ?? "已通过") : (a.appealRejected ?? "已驳回")}
                      </span>
                    )}
                  </p>
                  <p className="text-xs text-sub">
                    {ap.body}
                    {ap.result_note ? ` · ${a.appealNoteLabel}: ${ap.result_note}` : ""}
                  </p>
                </div>
                {ap.status === "open" && (
                  <span className="flex gap-2">
                    <button
                      onClick={() => handleAppeal(ap.id, true)}
                      className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white"
                    >
                      {a.appealAccept ?? "通过"}
                    </button>
                    <button
                      onClick={() => handleAppeal(ap.id, false)}
                      className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger"
                    >
                      {a.appealReject ?? "驳回"}
                    </button>
                  </span>
                )}
              </li>
            ))}
            {appeals.length === 0 && (
              <li className="py-6 text-center text-sub">{a.appealEmpty ?? "暂无申诉"}</li>
            )}
          </ul>
        </section>
      )}

      {tab === "users" && <AdminUsers classes={a.classList} />}

      {tab === "torrents" && <AdminTorrents />}

      {tab === "p2tools" && <AdminP2Tools />}


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
        <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
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
