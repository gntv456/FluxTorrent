"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
// C 批：StaffTools 改从 admin-tool-switch 的 lazy 壳取（同文件 renderTool
// 的 STAFF_TOOL_TABS 分支与这里共用一份分割件），避免 35+ 工具面板全量
// 进 admin 路由入口（W-33 同法）。
import { AdminShell, type PanelEntry } from "@/components/admin-shell";
import type { ToolTab } from "@/components/staff-tools";
import { useReviewQueue } from "./_parts/use-review-queue";
import { AdminOverviewPanel } from "./_parts/admin-overview-panel";
import { AppealsPanel, ReviewsPanel } from "./_parts/admin-queue-panels";
import { AuditListPanel, CheatersPanel } from "./_parts/admin-tool-panels";
import {
  renderSimpleTool,
  STAFF_TOOL_TABS,
  StaffToolsLazy as StaffTools,
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

const MSG_CLS = "mb-3 rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink";

/** ApiError → dict.errors[code] ?? message，否则 actionFailed */
function errText(
  e: unknown,
  a: Record<string, string>,
  dict: ReturnType<typeof useI18n>["dict"],
): string {
  return e instanceof ApiError
    ? (apiErrorMessage(dict, e))
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
  // 0288：审核队列（分页 + 拒因字典）状态在 ./_parts/use-review-queue.ts
  const rq = useReviewQueue();
  // P2 清偿：审核/申诉的 prompt+confirm 全部收编到这一个状态机——
  // prompt 语义走输入槽（label），confirm 语义直接挂 run；danger 按场景给。
  const ask = useConfirm();
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
      setCheaters([]); setMsg(dict.common.loadFailed);
    }
  }, [dict]);

  /** 核心四件：导航条目 + 徽章计数来源。
   *  徽章常驻左侧导航（reviews / reports / appeals），所以这四个任何工具页都需要。
   *  概览与审计的明细（audit / stats）**不在这里**，见下面的按需 effect。 */
  const load = useCallback(async () => {
    try {
      const [ovr, pnl, aps] = await Promise.all([
        api.get<Overview>("/api/v1/admin/overview"),
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
    api
      .get<AuditRow[]>("/api/v1/admin/audit")
      .then(setAudit)
      .catch(() => {});
    if (tool !== "overview") return;
    api
      .get<StatsData>("/api/v1/admin/stats")
      .then(setStats)
      .catch(() => {});
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
    // 0286：选了字典拒因就不再手打自由文本——结构化拒因才能统计、才能三语下发
    const useDict = !approve && rq.reasonId !== null;
    // P2 清偿：拒因采集（prompt → ConfirmDialog 输入槽；通过/字典拒因免输入）
    if (approve || useDict) {
      await doDecide(torrentId, approve, "");
      return;
    }
    ask.open({
      title: a.reject,
      label: a.rejectReason,
      run: (reason) => {
        if (reason.trim()) void doDecide(torrentId, approve, reason.trim());
      },
    });
  }

  async function doDecide(torrentId: number, approve: boolean, reason: string) {
    const useDict = !approve && rq.reasonId !== null;
    try {
      await api.post("/api/v1/admin/reviews/decide", {
        torrent_id: torrentId,
        approve,
        reason,
        ...(useDict ? { deny_reason_id: rq.reasonId } : {}),
      });
      setMsg(
        approve
          ? fmt(a.approved, { id: torrentId })
          : fmt(a.rejected, { id: torrentId }),
      );
      load();
      rq.reload().catch(() => {});
    } catch (e) {
      setMsg(errText(e, a, dict));
    }
  }

  /** 0306：暂缓（证据不足挂起）——后端 /admin/reviews/postpone 早已就绪，此前无 UI 入口。
   *  P2 清偿：暂缓理由采集走 ConfirmDialog 输入槽（prompt 退役）。 */
  function postpone(torrentId: number) {
    ask.open({
      title: a.postpone ?? a.reject,
      label: a.postponeReason ?? "postpone-reason",
      run: (reason) => void doPostpone(torrentId, reason),
    });
  }

  async function doPostpone(torrentId: number, reason: string) {
    try {
      await api.post("/api/v1/admin/reviews/postpone", {
        torrent_id: torrentId,
        reason,
      });
      setMsg(fmt(a.postponed, { id: torrentId }));
      load();
      rq.reload().catch(() => {});
    } catch (e) {
      setMsg(errText(e, a, dict));
    }
  }

  /** 0306：批量裁决本页待审（自审条目后端逐条跳过并在回执里说明原因）。
   *  P2 清偿：拒因 prompt + 批量 confirm 收编为一个状态机。 */
  function batchDecide(ids: number[], approve: boolean) {
    const useDict = !approve && rq.reasonId !== null;
    if (approve || useDict) {
      ask.open({
        title: approve ? a.approve : a.reject,
        message: fmt(a.batchConfirm, {
          n: ids.length,
          verdict: approve ? a.approve : a.reject,
        }),
        tone: "danger",
        run: () => void doBatch(ids, approve, ""),
      });
      return;
    }
    ask.open({
      title: a.reject,
      message: fmt(a.batchConfirm, {
        n: ids.length,
        verdict: approve ? a.approve : a.reject,
      }),
      tone: "danger",
      label: a.rejectReason,
      run: (reason) => {
        if (reason.trim()) void doBatch(ids, approve, reason.trim());
      },
    });
  }

  async function doBatch(ids: number[], approve: boolean, reason: string) {
    const useDict = !approve && rq.reasonId !== null;
    try {
      const r = await api.post<{
        handled: number;
        requested: number;
        skipped: { id: number; why: string }[];
      }>("/api/v1/admin/reviews/batch", {
        torrent_ids: ids,
        approve,
        reason,
        ...(useDict ? { deny_reason_id: rq.reasonId } : {}),
      });
      setMsg(
        fmt(a.batchDone, {
          n: r.handled,
          skipped: r.skipped?.length ?? 0,
        }),
      );
      load();
      rq.reload().catch(() => {});
    } catch (e) {
      setMsg(errText(e, a, dict));
    }
  }

  /** 申诉处理（P2 清偿）：批注 prompt → 输入槽；受理后的联动解封
   *  confirm（danger：解封即恢复账号）也走同一状态机，串成两步。 */
  function handleAppeal(id: number, accept: boolean) {
    const label = accept
      ? (a.appealAcceptNote ?? "accept-note")
      : (a.appealRejectNote ?? "reject-reason");
    ask.open({
      title: accept ? a.appealAccept : a.appealReject,
      label,
      run: (note) => {
        if (!accept && !note.trim()) return;
        if (!accept) {
          void doAppeal(id, false, note.trim(), false);
          return;
        }
        // 受理：先问是否联动解封（danger）
        ask.open({
          title: a.appealUnbanConfirm ?? "unban-confirm",
          message: a.appealUnbanConfirm ?? "unban-confirm",
          tone: "danger",
          run: () => void doAppeal(id, true, note.trim(), true),
        });
        void doAppeal(id, true, note.trim(), false);
      },
    });
  }

  async function doAppeal(
    id: number,
    accept: boolean,
    note: string,
    unban: boolean,
  ) {
    try {
      const r = await api.post<{ handled: number; unbanned: boolean }>(
        "/api/v1/admin/appeals/handle",
        {
          appeal_id: id,
          accept,
          note,
          unban,
        },
      );
      setMsg(
        fmt(a.appealHandled, { id }) +
          (r.unbanned ? ` · ${a.appealUnbanned ?? "unbanned"}` : ""),
      );
      load();
    } catch (e) {
      setMsg(errText(e, a, dict));
    }
  }

  const badges: Record<string, number> = {
    reviews: rq.total,
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
            reviews={rq.items}
            appeals={appeals}
            audit={audit}
            onOpen={handleTool}
          />
        );

      case "reviews":
        return (
          <ReviewsPanel
            reviews={rq.items}
            total={rq.total}
            offset={rq.offset}
            onPage={rq.setPage}
            denyReasons={rq.reasons}
            denyReasonId={rq.reasonId}
            onDenyReason={rq.setReasonId}
            onDecide={decide}
            onPostpone={postpone}
            onBatch={batchDecide}
          />
        );

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
      {/* 顶栏标题降为 p 后，工具页的唯一 h1 在此（对读屏可见即可） */}
      <h1 className="sr-only">{a.panelTitle}</h1>
      {msg && <p className={MSG_CLS}>{msg}</p>}
      {renderTool()}
      <AskDialog {...ask} />
    </AdminShell>
  );
}
