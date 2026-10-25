"use client";

/**
 * 审核台工具栏（从 admin-queue-panels.tsx 按域拆出，300 行门禁）：
 * 待审/暂缓双视图切换 + 拒因字典 + 批量裁决 + 分页。
 * 判定动作（approve/reject/hold/resume）留在 ReviewsPanel 行内。
 */

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { DenyReason } from "@/components/admin-torrents-shared";

export function ReviewQueueToolbar({
  total,
  offset,
  pageSize,
  itemCount,
  denyReasons,
  denyReasonId,
  onDenyReason,
  onBatch,
  onPage,
  status,
  onStatus,
}: {
  total: number;
  offset: number;
  pageSize: number;
  /** 本页条目数：0 时禁批量按钮（与旧口径一致，免打空批量吃 400） */
  itemCount: number;
  denyReasons: DenyReason[];
  denyReasonId: number | null;
  onDenyReason: (id: number | null) => void;
  onBatch: (approve: boolean) => void;
  onPage: (next: number) => void;
  /** 0306：审核态视图 0=待审 / 4=暂缓 */
  status: number;
  onStatus: (s: number) => void;
}) {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const postponed = status === 4;
  // 两个批量按钮类名拆行拼：单行超 80 列门禁（行宽按 UTF-16 码元计）
  const batchPassCls =
    "min-h-[36px] rounded-full border border-mint px-3 py-1 " +
    "font-bold text-mint disabled:opacity-40";
  const batchDenyCls =
    "min-h-[36px] rounded-full border border-coral px-3 py-1 " +
    "font-bold text-danger disabled:opacity-40";
  return (
    <div className="mb-2 flex flex-wrap items-center gap-2 text-xs">
      <span className="text-sub">{fmt(a.queueTotal, { n: total })}</span>
      {/* 0306：待审/暂缓双视图。暂缓只是挂起不是终态——
          旧版暂缓后种从台上消失，审核员再也找不回它 */}
      <span className="flex gap-1">
        <button
          onClick={() => onStatus(0)}
          className={
            postponed
              ? "rounded border border-line px-2 py-1 text-sub"
              : "rounded border border-mint px-2 py-1 font-bold text-mint"
          }
        >
          {a.queueViewPending}
        </button>
        <button
          onClick={() => onStatus(4)}
          className={
            postponed
              ? "rounded border border-sun px-2 py-1 font-bold text-ink"
              : "rounded border border-line px-2 py-1 text-sub"
          }
        >
          {a.queueViewPostponed}
        </button>
      </span>
      {!postponed && (
        <>
          <select
            className="rounded border border-line bg-transparent px-2 py-1"
            value={denyReasonId ?? ""}
            onChange={(e) =>
              onDenyReason(e.target.value ? Number(e.target.value) : null)
            }
          >
            <option value="">{a.denyPickNone}</option>
            {denyReasons.map((r) => (
              <option key={r.id} value={r.id}>
                {r.reason}
              </option>
            ))}
          </select>
          {/* 0306：批量裁决后端早已就绪（0288），积压时审核员此前仍只能逐条点 */}
          <span className="flex gap-1">
            <button
              disabled={itemCount === 0}
              onClick={() => onBatch(true)}
              className={batchPassCls}
            >
              {a.batchApprove}
            </button>
            <button
              disabled={itemCount === 0}
              onClick={() => onBatch(false)}
              className={batchDenyCls}
            >
              {a.batchReject}
            </button>
          </span>
        </>
      )}
      <span className="ml-auto flex gap-1">
        <button
          disabled={offset <= 0}
          onClick={() => onPage(Math.max(0, offset - pageSize))}
          className="rounded border border-line px-2 py-1 disabled:opacity-40"
        >
          {a.queuePrev}
        </button>
        <button
          disabled={offset + pageSize >= total}
          onClick={() => onPage(offset + pageSize)}
          className="rounded border border-line px-2 py-1 disabled:opacity-40"
        >
          {a.queueNext}
        </button>
      </span>
    </div>
  );
}
