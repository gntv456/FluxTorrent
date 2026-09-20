"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { ContentManage } from "@/components/content-manage";
import { StaffTools, type ToolTab } from "@/components/staff-tools";
import { AdminUsers } from "@/components/admin-users";
import { AdminTorrents } from "@/components/admin-torrents";
import { AdminP2Tools } from "@/components/admin-p2-tools";
import { AdminShell, type PanelEntry } from "@/components/admin-shell";
import { AdminHr } from "@/components/admin-hr";
import { AdminInvites } from "@/components/admin-invites";
import { AdminUserLogs } from "@/components/admin-userlogs";
import { AdminAttendance } from "@/components/admin-attendance";
import { AdminTagDict } from "@/components/admin-tagdict";
import { AdminSections } from "@/components/admin-sections";
import { AdminMedals } from "@/components/admin-medals";
import { AdminProps } from "@/components/admin-props";
import { AdminExams } from "@/components/admin-exams";
import { AdminJixiao } from "@/components/admin-jixiao";
import { AdminTasks } from "@/components/admin-tasks";
import { AdminTrackers } from "@/components/admin-trackers";
import { AdminOpsPanel } from "@/components/admin-ops";
import { HomeLayoutEditor } from "@/components/home-layout-editor";
import { AdminOverviewPanel } from "./_parts/admin-overview-panel";
import { AppealsPanel, ReviewsPanel } from "./_parts/admin-queue-panels";
import { ClearCachePanel, FreeleechPanel } from "./_parts/admin-freeleech";
import { AuditListPanel, CheatersPanel } from "./_parts/admin-tool-panels";
import {
  LEGACY_TOOL,
  type AppealRow,
  type AuditRow,
  type CheaterRow,
  type Overview,
  type PendingTorrent,
  type StatsData,
} from "./_parts/admin-shared";

/** 由 staff-tools 承载的工具页签（tab_key 与 ToolTab 同名） */
const STAFF_TOOL_TABS: ToolTab[] = [
  "faq", "rules", "cats", "bans", "mail", "promo", "staffmess", "adduser",
  "incrementbulk", "warned", "ipcheck", "maxlogin", "resetpass", "deldisabled",
  "emailbans", "testip", "stats", "cleanup", "ads", "notconnect", "uploaders",
  "agents", "polls", "dbstats", "syslog", "locations", "hrpardon", "plugins",
  "agentrules", "forums", "reports", "menu", "roles", "perm", "seedstats",
];

/**
 * 管理后台（staffpanel + 管理系统）。
 *
 * 信息架构：按职能（section）分组 → 左侧常驻导航 → 内容区渲染对应工具。
 * 旧的「权限桶分组 + 四行药丸带 + 三组卡片网格」已移除，避免同一功能多处入口。
 * 总览/队列/工具面板拆至 ./_parts/（300 行门禁）。
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
      case "overview":
        return (
          <AdminOverviewPanel
            ov={ov}
            stats={stats}
            reviews={reviews}
            appeals={appeals}
            audit={audit}
            onOpen={handleTool}
          />
        );

      case "reviews":
        return <ReviewsPanel reviews={reviews} onDecide={decide} />;

      case "appeals":
        return <AppealsPanel appeals={appeals} onHandle={handleAppeal} />;

      case "cheaters":
        return <CheatersPanel cheaters={cheaters} onScan={loadCheaters} />;

      case "audit":
        return <AuditListPanel audit={audit} />;

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
      // 第八轮 P3 套件（好学站后台逐页深挖落地）
      case "hr":
        return <AdminHr />;
      case "invites":
        return <AdminInvites />;
      case "userlogs":
        return <AdminUserLogs />;
      case "attendance":
        return <AdminAttendance />;
      case "tagdict":
        return <AdminTagDict />;
      case "sections":
        return <AdminSections />;
      case "medals":
        return <AdminMedals />;
      case "props":
        return <AdminProps />;
      case "exams":
        return <AdminExams />;
      case "jixiao":
        return <AdminJixiao />;
      case "tasks":
        return <AdminTasks />;
      case "trackers":
        return <AdminTrackers />;
      // 运维三件套（0078）：版本信息 / 备份面板 / 任务手动触发
      case "ops":
        return <AdminOpsPanel />;
      // 首页排版（0089）：板块顺序/宽度/显隐可视化编辑
      case "homelayout":
        return (
          <section className="baozi-panel p-4">
            <h2 className="mb-3 font-display text-lg">{dict.homeLayout.title}</h2>
            <HomeLayoutEditor />
          </section>
        );

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
