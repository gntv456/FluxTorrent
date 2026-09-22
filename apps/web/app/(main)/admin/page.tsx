"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { StaffTools, type ToolTab } from "@/components/staff-tools";
import { AdminShell, type PanelEntry } from "@/components/admin-shell";
import { AdminOverviewPanel } from "./_parts/admin-overview-panel";
import { AppealsPanel, ReviewsPanel } from "./_parts/admin-queue-panels";
import { AuditListPanel, CheatersPanel } from "./_parts/admin-tool-panels";
import {
  renderSimpleTool,
  STAFF_TOOL_TABS,
} from "./_parts/admin-tool-switch";
import {
  LEGACY_TOOL,
  type AppealRow,
  type AuditRow,
  type CheaterRow,
  type Overview,
  type PendingTorrent,
  type StatsData,
} from "./_parts/admin-shared";

const MSG_CLS =
  "mb-3 rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink";

/** ApiError → dict.errors[code] ?? message，否则 actionFailed */
function errText(
  e: unknown,
  a: Record<string, string>,
  dict: ReturnType<typeof useI18n>["dict"],
): string {
  return e instanceof ApiError
    ? (dict.errors[e.code] ?? e.message)
    : a.actionFailed;
}

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
  //
  // ⚠️ `tool` 的初始值是 "overview"，URL 派生的值要在本 effect 之后才写入。
  // 下面「按需拉取」的 effect 依赖 tool —— 若不加 toolReady 闸门，它会在首轮
  // 以 tool="overview" 抢先拉一次 audit/stats，`?tool=faq` 就白拉了。
  const [toolReady, setToolReady] = useState(false);
  useEffect(() => {
    const raw = new URLSearchParams(window.location.search).get("tool");
    if (raw) setTool(LEGACY_TOOL[raw] ?? raw);
    setToolReady(true);
  }, []);

  /** 深链兜底：DB 中 url 不以 `/admin?tool=` 开头的条目（站点设定 → /admin/settings、
   *  论坛结构 → /admin/forums）在 handleTool 里走整页跳转，但**直接加载**
   *  `?tool=settings` 会绕过 handleTool、落进 panelEmpty 空面板。拿到导航条目后
   *  补一次同样的跳转（replace 不留历史，避免后退回到空面板）。 */
  useEffect(() => {
    if (entries.length === 0) return;
    const e = entries.find((x) => x.tab_key === tool);
    if (e && !e.url.startsWith("/admin?tool=")) {
      window.location.replace(e.url);
    }
  }, [entries, tool]);

  const loadCheaters = useCallback(async () => {
    try {
      setCheaters(await api.get<CheaterRow[]>("/api/v1/admin/cheaters"));
    } catch {
      setCheaters([]);
    }
  }, []);

  /** 核心四件：导航条目 + 徽章计数来源。
   *  徽章常驻左侧导航（reviews / reports / appeals），所以这四个任何工具页都需要。
   *  概览与审计的明细（audit / stats）**不在这里**，见下面的按需 effect。 */
  const load = useCallback(async () => {
    try {
      const [ovr, rev, pnl, aps] = await Promise.all([
        api.get<Overview>("/api/v1/admin/overview"),
        api.get<PendingTorrent[]>("/api/v1/admin/reviews"),
        api.get<{
          entries: PanelEntry[];
          role: string;
          class_id?: number;
        }>("/api/v1/admin/staffpanel"),
        api
          .get<AppealRow[]>("/api/v1/admin/appeals")
          .catch(() => [] as AppealRow[]),
      ]);
      setOv(ovr);
      setReviews(rev);
      setEntries(pnl.entries);
      setRole(pnl.role);
      setClassId(pnl.class_id);
      setAppeals(aps);
    } catch (e) {
      setMsg(
        e instanceof ApiError && e.code === 2003
          ? a.needAdmin
          : dict.common.loadFailed,
      );
    }
  }, [a, dict]);

  /** 概览/审计的明细按需拉取。
   *  此前 audit 与 stats 在 load() 里**无条件**拉，导致每个工具页多两个请求
   *  （实测 ?tool=faq 共 13 个端点，其中这 2 个与该工具无关）。
   *  `toolReady` 是必须的：否则首轮 tool 还是默认的 "overview"，会白拉一次。 */
  useEffect(() => {
    if (!toolReady) return;
    if (tool !== "overview" && tool !== "audit") return;
    api.get<AuditRow[]>("/api/v1/admin/audit").then(setAudit).catch(() => {});
    if (tool !== "overview") return;
    api.get<StatsData>("/api/v1/admin/stats").then(setStats).catch(() => {});
  }, [tool, toolReady]);

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
      window.history.replaceState(
        null,
        "",
        `/admin?tool=${encodeURIComponent(t)}`,
      );
    },
    [entries],
  );

  async function decide(torrentId: number, approve: boolean) {
    const reason = approve ? "" : (prompt(a.rejectReason) ?? "");
    if (!approve && !reason) return;
    try {
      await api.post("/api/v1/admin/reviews/decide", {
        torrent_id: torrentId,
        approve,
        reason,
      });
      setMsg(
        approve
          ? fmt(a.approved, { id: torrentId })
          : fmt(a.rejected, { id: torrentId }),
      );
      load();
    } catch (e) {
      setMsg(errText(e, a, dict));
    }
  }

  async function handleAppeal(id: number, accept: boolean) {
    const note =
      prompt(
        accept
          ? (a.appealAcceptNote ?? "accept-note")
          : (a.appealRejectNote ?? "reject-reason"),
      ) ?? "";
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
      setMsg(errText(e, a, dict));
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

      default: {
        const simple = renderSimpleTool(tool, a, dict);
        if (simple !== undefined) return simple;
        if (STAFF_TOOL_TABS.includes(tool as ToolTab)) {
          return <StaffTools initialTab={tool as ToolTab} />;
        }
        return <p className="py-8 text-center text-sub">{a.panelEmpty}</p>;
      }
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
      {msg && (
        <p className={MSG_CLS}>{msg}</p>
      )}
      {renderTool()}
    </AdminShell>
  );
}
