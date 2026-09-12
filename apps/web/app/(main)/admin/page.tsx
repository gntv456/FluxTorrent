"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { ContentManage } from "@/components/content-manage";
import { StaffTools, type ToolTab } from "@/components/staff-tools";
import { AdminUsers } from "@/components/admin-users";
import { AdminTorrents } from "@/components/admin-torrents";
import { AdminP2Tools } from "@/components/admin-p2-tools";
import { AdminShell, type PanelEntry } from "@/components/admin-shell";

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

interface AuditRow {
  id: number;
  actor_id: number | null;
  action: string;
  created_at: string;
}

interface StatsData {
  users: number; torrents: number; seeding: number; leeching: number;
  comments: number; messages: number; redis: string; db: string; uptime_secs: number;
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

interface PromoRow {
  id: number;
  scope: string;
  kind: string;
  category_id: number | null;
  category_name: string | null;
  starts_at: string;
  ends_at: string;
}

/** 由 staff-tools 承载的工具页签（tab_key 与 ToolTab 同名） */
const STAFF_TOOL_TABS: ToolTab[] = [
  "faq", "rules", "cats", "bans", "mail", "promo", "staffmess", "adduser",
  "bonus", "warned", "ipcheck", "maxlogin", "upload", "resetpass", "deldisabled",
  "emailbans", "testip", "stats", "cleanup", "ads", "notconnect", "uploaders",
  "agents", "polls", "dbstats", "syslog", "locations", "hrpardon", "plugins",
  "agentrules", "forums", "reports", "menu", "roles", "perm", "seedstats",
];

/** 旧 URL 兼容：历史 ?tool= 值与现 tab_key 命名不一致，映射后旧书签不失效 */
const LEGACY_TOOL: Record<string, string> = {
  reset: "resetpass",
  deletedisabled: "deldisabled",
  bannedemails: "emailbans",
  allowedemails: "emailbans",
  docleanup: "cleanup",
  admanage: "ads",
  allagents: "agents",
  polloverview: "polls",
  amountbonus: "bonus",
  amountupload: "upload",
  notconnectable: "notconnect",
  location: "locations",
  faqmanage: "faq",
  modrules: "rules",
  catmanage: "cats",
  mysql_stats: "dbstats",
  bitbucketlog: "syslog",
};

const PROMO_KINDS: [string, string][] = [
  ["free", "免费下载"],
  ["x2", "双倍上传"],
  ["x2free", "免费 + 双倍"],
  ["half", "半价下载"],
  ["x2half", "半价 + 双倍"],
  ["p30", "30% 下载"],
];

const PROMO_SCOPES: [string, string][] = [
  ["global", "全站"],
  ["official", "官方种"],
  ["non_official", "非官方种"],
  ["category", "指定分类"],
];

/** 促销状态（freeleech）管理：对应 /admin/freeleech 的增删查 */
function FreeleechPanel() {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [rows, setRows] = useState<PromoRow[]>([]);
  const [kind, setKind] = useState("free");
  const [scope, setScope] = useState("global");
  const [hours, setHours] = useState(24);
  const [categoryId, setCategoryId] = useState("1");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.get<PromoRow[]>("/api/v1/admin/freeleech"));
    } catch {
      setRows([]);
    }
  }, []);

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
        ...(scope === "category" ? { category_id: Number(categoryId) } : {}),
      });
      setMsg("促销已生效");
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: number) {
    setBusy(true);
    try {
      await api.del(`/api/v1/admin/freeleech/${id}`);
      setMsg("已取消");
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky";

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">促销状态</h2>
      <p className="mb-3 text-xs text-sub">
        对全站或指定范围种子设置免费 / 双倍 / 半价状态，到期自动失效。
      </p>
      {msg && <p className="mb-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs">{msg}</p>}
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead w-24">类型</td>
              <td className="colhead w-28">范围</td>
              <td className="colhead">开始</td>
              <td className="colhead">结束</td>
              <td className="colhead w-20" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="rowfollow">
                  {PROMO_KINDS.find(([k]) => k === r.kind)?.[1] ?? r.kind}
                </td>
                <td className="rowfollow">
                  {PROMO_SCOPES.find(([s]) => s === r.scope)?.[1] ?? r.scope}
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
                    onClick={() => remove(r.id)}
                    className="min-h-[28px] rounded-full border border-line px-3 font-bold text-danger disabled:opacity-50"
                  >
                    取消
                  </button>
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={5} className="py-4 text-center text-sub">
                  当前没有进行中的促销
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-4 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">类型</span>
          <select value={kind} onChange={(e) => setKind(e.target.value)} className={inputCls}>
            {PROMO_KINDS.map(([k, label]) => (
              <option key={k} value={k}>{label}</option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">范围</span>
          <select value={scope} onChange={(e) => setScope(e.target.value)} className={inputCls}>
            {PROMO_SCOPES.map(([s, label]) => (
              <option key={s} value={s}>{label}</option>
            ))}
          </select>
        </label>
        {scope === "category" && (
          <label className="flex flex-col gap-1">
            <span className="text-xs text-sub">分类 ID</span>
            <input
              type="number"
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
              className={`${inputCls} w-24`}
            />
          </label>
        )}
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">时长（小时）</span>
          <input
            type="number"
            min={1}
            max={720}
            value={hours}
            onChange={(e) => setHours(Number(e.target.value))}
            className={`${inputCls} w-28`}
          />
        </label>
        <button
          disabled={busy}
          onClick={create}
          className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
        >
          生效
        </button>
      </div>
    </section>
  );
}

/** 清除缓存：对应 POST /admin/clearcache（清除限流等运行期缓存键） */
function ClearCachePanel() {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run() {
    setBusy(true);
    try {
      const r = await api.post<{ cleared: number }>("/api/v1/admin/clearcache", {});
      setMsg(`已清除 ${r.cleared} 个缓存键`);
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">清除缓存</h2>
      <p className="mb-3 text-xs text-sub">
        清除运行期缓存键（限流计数等）。不影响数据库数据，站点会自动重建缓存。
      </p>
      {msg && <p className="mb-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs">{msg}</p>}
      <button
        disabled={busy}
        onClick={run}
        className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
      >
        立即清除
      </button>
    </section>
  );
}

/**
 * 管理后台（staffpanel + 管理系统）。
 *
 * 信息架构：按职能（section）分组 → 左侧常驻导航 → 内容区渲染对应工具。
 * 旧的「权限桶分组 + 四行药丸带 + 三组卡片网格」已移除，避免同一功能多处入口。
 */
export default function AdminPage() {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [entries, setEntries] = useState<PanelEntry[]>([]);
  const [role, setRole] = useState("");
  const [classId, setClassId] = useState<number | undefined>(undefined);
  const [tool, setTool] = useState("overview");
  const [cheaters, setCheaters] = useState<CheaterRow[]>([]);
  const [ov, setOv] = useState<Overview | null>(null);
  const [reviews, setReviews] = useState<PendingTorrent[]>([]);
  const [appeals, setAppeals] = useState<AppealRow[]>([]);
  const [audit, setAudit] = useState<AuditRow[]>([]);
  const [stats, setStats] = useState<StatsData | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  // 初始工具：读 URL ?tool=，并把历史命名映射到新 tab_key（旧书签不失效）
  useEffect(() => {
    const raw = new URLSearchParams(window.location.search).get("tool");
    if (!raw) return;
    setTool(LEGACY_TOOL[raw] ?? raw);
  }, []);

  const loadCheaters = useCallback(async () => {
    try {
      setCheaters(await api.get<CheaterRow[]>("/api/v1/admin/cheaters"));
    } catch {
      setCheaters([]);
    }
  }, []);

  const load = useCallback(async () => {
    try {
      const [ovr, rev, aud, pnl, aps, sts] = await Promise.all([
        api.get<Overview>("/api/v1/admin/overview"),
        api.get<PendingTorrent[]>("/api/v1/admin/reviews"),
        api.get<AuditRow[]>("/api/v1/admin/audit"),
        api.get<{ entries: PanelEntry[]; role: string; class_id?: number }>("/api/v1/admin/staffpanel"),
        api.get<AppealRow[]>("/api/v1/admin/appeals").catch(() => [] as AppealRow[]),
        api.get<StatsData>("/api/v1/admin/stats").catch(() => null),
      ]);
      setOv(ovr);
      setReviews(rev);
      setAudit(aud);
      setEntries(pnl.entries);
      setRole(pnl.role);
      setClassId(pnl.class_id);
      setAppeals(aps);
      setStats(sts);
    } catch (e) {
      setMsg(e instanceof ApiError && e.code === 2003 ? a.needAdmin : dict.common.loadFailed);
    }
  }, [a, dict]);

  useEffect(() => {
    load();
  }, [load]);

  /** 切换工具：更新 URL（可深链、可分享）；外链型条目（如站点设定）直接跳转 */
  const handleTool = useCallback(
    (t: string) => {
      const entry = entries.find((e) => e.tab_key === t);
      if (entry && !entry.url.startsWith("/admin?tool=")) {
        window.location.href = entry.url;
        return;
      }
      setTool(t);
      window.history.replaceState(null, "", `/admin?tool=${encodeURIComponent(t)}`);
    },
    [entries],
  );

  async function decide(torrentId: number, approve: boolean) {
    const reason = approve ? "" : (prompt(a.rejectReason) ?? "");
    if (!approve && !reason) return;
    try {
      await api.post("/api/v1/admin/reviews/decide", { torrent_id: torrentId, approve, reason });
      setMsg(approve ? fmt(a.approved, { id: torrentId }) : fmt(a.rejected, { id: torrentId }));
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    }
  }

  async function handleAppeal(id: number, accept: boolean) {
    const note =
      prompt(
        accept
          ? (a.appealAcceptNote ?? "通过说明（可选）")
          : (a.appealRejectNote ?? "驳回理由（必填）"),
      ) ?? "";
    if (!accept && !note.trim()) return;
    try {
      await api.post("/api/v1/admin/appeals/handle", { appeal_id: id, accept, note });
      setMsg(fmt(a.appealHandled, { id }));
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : a.actionFailed);
    }
  }

  const badges: Record<string, number> = {
    reviews: reviews.length,
    reports: ov?.open_reports ?? 0,
    // 申诉也是待办队列，此前只有 reviews/reports 有徽标
    appeals: appeals.filter((x) => x.status === "open").length,
  };

  function renderTool() {
    switch (tool) {
      case "overview": {
        const openAppeals = appeals.filter((x) => x.status === "open").length;
        const uptimeH = stats ? Math.max(1, Math.round(stats.uptime_secs / 3600)) : 0;
        const queue = [
          { label: a.pendingReviews, v: reviews.length, to: "reviews", warn: reviews.length > 0 },
          { label: a.openReports, v: ov?.open_reports ?? 0, to: "reports", warn: (ov?.open_reports ?? 0) > 0 },
          { label: "待处理申诉", v: openAppeals, to: "appeals", warn: openAppeals > 0 },
          { label: "作弊探测", v: null as number | null, to: "cheaters", warn: false },
        ];
        const site = stats
          ? ([
              [a.users, stats.users], ["种子", stats.torrents],
              ["做种中", stats.seeding], ["下载中", stats.leeching],
              ["评论", stats.comments], ["站内信", stats.messages],
            ] as [string, number][])
          : [];
        return (
          <section className="flex flex-col gap-4">
            <div>
              <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">待办队列</h2>
              <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
                {queue.map((s) => (
                  <button
                    key={s.label}
                    onClick={() => handleTool(s.to)}
                    title="点击打开对应队列"
                    className={`rounded-[var(--r-md)] border bg-[var(--surface-card)] p-3 text-center shadow-[var(--shadow-card)] transition hover:border-sky ${
                      s.warn ? "border-danger/40" : "border-line"
                    }`}
                  >
                    <p className="text-xs text-sub">{s.label}</p>
                    <p className={`num mt-1 text-2xl ${s.warn ? "text-danger" : "text-sky"}`}>
                      {s.v ?? "—"}
                    </p>
                  </button>
                ))}
              </div>
            </div>

            {stats && (
              <div>
                <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">站点数据</h2>
                <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-6">
                  {site.map(([label, v]) => (
                    <div
                      key={label}
                      className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3 text-center"
                    >
                      <p className="text-xs text-sub">{label}</p>
                      <p className="num mt-1 text-xl text-ink">{v.toLocaleString()}</p>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {stats && (
              <div className="flex flex-wrap items-center gap-x-5 gap-y-1 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-4 py-2.5 text-xs text-sub">
                <span>
                  数据库{" "}
                  <b className={stats.db === "up" ? "text-mint" : "text-danger"}>
                    {stats.db === "up" ? "正常" : "异常"}
                  </b>
                </span>
                <span>
                  Redis{" "}
                  <b className={stats.redis === "up" ? "text-mint" : "text-danger"}>
                    {stats.redis === "up" ? "正常" : "异常"}
                  </b>
                </span>
                <span>API 已运行 {uptimeH} 小时</span>
              </div>
            )}

            <div>
              <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">
                {a.recentActions ?? "最近操作"}
              </h2>
              <div className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4">
                <ul className="flex flex-col divide-y divide-line text-sm">
                  {audit.slice(0, 6).map((row) => (
                    <li key={row.id} className="flex items-center justify-between py-2">
                      <span className="font-mono text-xs">{row.action}</span>
                      <span className="text-xs text-sub">
                        {fmt(a.actor, { id: row.actor_id ?? "-" })} ·{" "}
                        {new Date(row.created_at).toLocaleString(dateLocale(locale))}
                      </span>
                    </li>
                  ))}
                  {audit.length === 0 && (
                    <li className="py-4 text-center text-sub">{a.queueEmpty}</li>
                  )}
                </ul>
              </div>
            </div>
          </section>
        );
      }

      case "reviews":
        return (
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
              {reviews.length === 0 && (
                <li className="py-6 text-center text-sub">{a.queueEmpty}</li>
              )}
            </ul>
          </section>
        );

      case "appeals":
        return (
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
                      {ap.ref_id !== null && (
                        <span className="text-xs text-sub"> · #{ap.ref_id}</span>
                      )}
                      {ap.status !== "open" && (
                        <span
                          className={`ml-1 rounded-full px-2 py-0.5 text-[10px] ${
                            ap.status === "accepted" ? "bg-mint/30" : "bg-coral/20 text-danger"
                          }`}
                        >
                          {ap.status === "accepted"
                            ? (a.appealAccepted ?? "已通过")
                            : (a.appealRejected ?? "已驳回")}
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
        );

      case "cheaters":
        return (
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
                          <a href={`/torrent/${row.torrent_id}`}>
                            {row.name ?? `#${row.torrent_id}`}
                          </a>
                        ) : (
                          "—"
                        )}
                      </td>
                      <td className="rowfollow num">
                        {(row.upspeed / 1024 / 1024).toFixed(1)} MB/s
                      </td>
                      <td className="rowfollow num">
                        {(row.uploaded_delta / 1024 ** 3).toFixed(2)} GB
                      </td>
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
        );

      case "audit":
        return (
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
        );

      case "users":
        return <AdminUsers classes={dict.admin.classList} />;
      case "torrents":
        return <AdminTorrents />;
      case "content":
        return (
          <section className="nexus-detail">
            <h2 className="mb-3 text-base font-bold text-ink">{a.sectionContent ?? "内容"}</h2>
            <ContentManage />
          </section>
        );
      case "freeleech":
        return <FreeleechPanel />;
      case "clearcache":
        return <ClearCachePanel />;
      case "p2tools":
        return <AdminP2Tools />;

      default:
        if (STAFF_TOOL_TABS.includes(tool as ToolTab)) {
          return <StaffTools initialTab={tool as ToolTab} />;
        }
        return <p className="py-8 text-center text-sub">{a.panelEmpty}</p>;
    }
  }

  return (
    <AdminShell
      entries={entries}
      tool={tool}
      onTool={handleTool}
      badges={badges}
      role={role}
      classId={classId}
    >
      {msg && <p className="mb-3 rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
      {renderTool()}
    </AdminShell>
  );
}
