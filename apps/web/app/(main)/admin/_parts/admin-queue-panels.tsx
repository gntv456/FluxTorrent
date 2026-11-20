

"use client";

import { waitingText } from "./admin-appeals-panel";

export { AppealsPanel } from "./admin-appeals-panel";


import { PANEL_LG } from "@/lib/ui-classes";

/**
 * 管理后台待办队列面板（从 app/(main)/admin/page.tsx 按域拆出）：
 * 待审种子审核（ReviewsPanel）与申诉处理（AppealsPanel）。
 * 判定动作（approve/reject/handle）通过回调交还管理页。
 */

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { DenyReason } from "@/components/admin-torrents-shared";
import type { AppealRow, PendingTorrent } from "./admin-shared";
import { ReviewQueueToolbar } from "./review-queue-toolbar";


export function ReviewsPanel({
  reviews,
  total,
  offset,
  onPage,
  denyReasons,
  denyReasonId,
  onDenyReason,
  onDecide,
  onPostpone,
  onBatch,
  onResume,
  status,
  onStatus,
  failed,
  onRetry,
}: {
  reviews: PendingTorrent[];
  total: number;
  offset: number;
  onPage: (next: number) => void;
  denyReasons: DenyReason[];
  denyReasonId: number | null;
  onDenyReason: (id: number | null) => void;
  onDecide: (torrentId: number, approve: boolean) => void;
  /** 暂缓（证据不足挂起，POST /admin/reviews/postpone；0306 补接线） */
  onPostpone: (torrentId: number) => void;
  /** 批量通过本页全部待审（POST /admin/reviews/batch；自审条目由后端跳过） */
  onBatch: (ids: number[], approve: boolean) => void;
  /** P0-4：队列取数失败，不能被读成「队列为空」 */
  failed: boolean;
  onRetry: () => void;
  /** 暂缓恢复（POST /admin/reviews/resume；暂缓视图专用） */
  onResume: (torrentId: number) => void;
  /** 0306：审核态视图 0=待审 / 4=暂缓 */
  status: number;
  onStatus: (s: number) => void;
}) {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const pageSize = reviews.length || 50;
  const pageIds = reviews.map((t) => t.id);
  const postponed = status === 4;
  // 恢复按钮类名拆两行拼：单行会超 80 列门禁（行宽按 UTF-16 码元）
  // B4-11：操作按钮统一用 ux-tap（min-height: var(--tap-min)），
  // 按钮区 ux-review-actions 在窄屏占满一行并 flex-wrap，防横向溢出。
  const resumeCls =
    "ux-tap rounded-full border border-sun px-4 " +
    "text-xs font-bold text-ink";
  return (
    <section className={PANEL_LG}>
      <ReviewQueueToolbar
        total={total}
        offset={offset}
        pageSize={pageSize}
        itemCount={pageIds.length}
        denyReasons={denyReasons}
        denyReasonId={denyReasonId}
        onDenyReason={onDenyReason}
        onBatch={(approve) => onBatch(pageIds, approve)}
        onPage={onPage}
        status={status}
        onStatus={onStatus}
      />
      <ul className="flex flex-col divide-y divide-line">
        {reviews.map((t) => (
          <li
              key={t.id}
              className="flex flex-wrap items-start gap-2 py-2 sm:gap-3"
            >
            <div className="w-full min-w-0 sm:w-auto sm:flex-1">
              <p className="text-sm font-bold">{t.name}</p>
              <p className="text-xs text-sub">
                #{t.id} ·{" "}
                {fmt(a.queueWaiting, { t: waitingText(t.created_at) })} ·{" "}
                {fmt(a.uploader, {
                  name: t.owner_name ?? t.owner_id ?? dict.torrent.anonymous,
                })}{" "}
                · {(t.size / 1024 / 1024 / 1024).toFixed(2)}GB
                {t.category_name ? ` · ${t.category_name}` : ""}
              </p>
              {/* 0285：内容面可见——文件数/截图/NFO/查重命中/上传者过审与驳回史 */}
              <p className="text-xs text-sub">
                {t.numfiles ?? 0}F · {t.screenshots ?? 0}IMG
                {t.has_nfo ? " · NFO" : ""}
                {t.has_media_info ? " · MEDIA" : ""}
                {(t.dup_hash ?? 0) > 0 ? ` · DUP×${t.dup_hash}` : ""}
                {(t.dup_name ?? 0) > 0 ? ` · NAME×${t.dup_name}` : ""}
                {" · "}
                {t.owner_approved ?? 0}/
                {(t.owner_approved ?? 0) + (t.owner_denied ?? 0)}
              </p>
              {t.small_descr ? (
                <p className="mt-1 text-xs">{t.small_descr}</p>
              ) : null}
              {/* 0326 分型字段：季/话数/作者/联赛…按站型各取所需（后端早已
                  下发，这里补渲染——审影视看季集、审音乐看日志分） */}
              {t.sections &&
                Object.keys(t.sections).length > 0 && (
                  <p className="mt-1 text-xs">
                    {Object.entries(t.sections)
                      .map(
                        ([k, v]) =>
                          `${k}: ${(v.vs ?? []).filter(Boolean).join(" / ") ||
                          v.v ||
                          "—"}`,
                      )
                      .join(" · ")}
                  </p>
                )}
              {t.logs && (
                <p className="text-xs text-sub">
                  LOG {t.logs.min ?? "—"}分 × {t.logs.discs}碟
                </p>
              )}
              {t.artifacts && t.artifacts.length > 0 && (
                <p className="text-xs text-sub">
                  ART{" "}
                  {t.artifacts.map((x) => x.filename).join(" / ")}
                </p>
              )}
              {t.descr_excerpt ? (
                <p className="mt-1 line-clamp-3 text-xs text-sub">
                  {t.descr_excerpt.replace(/<[^>]+>/g, "")}
                </p>
              ) : null}
            </div>
            <div className="ux-review-actions">
            <a
              href={`/torrent/${t.id}`}
              target="_blank"
              rel="noreferrer"
              className="self-center text-xs underline text-sub"
            >
              {a.queueOpen}
            </a>
            {postponed ? (
              /* 暂缓视图：只能先恢复待审（decide 的 WHERE 谓词只认
                 status=0，直接过审/拒审会被后端挡回）；恢复后回到
                 待审视图即可常规裁决 */
              <button onClick={() => onResume(t.id)} className={resumeCls}>
                {a.resume}
              </button>
            ) : (
              <>
                <button
                  onClick={() => onDecide(t.id, true)}
                  className="ux-tap rounded-full bg-mint px-4 text-xs
                    font-bold text-[var(--text-on-accent)]"
                >
                  {a.approve}
                </button>
                <button
                  onClick={() => onDecide(t.id, false)}
                  className="ux-tap rounded-full bg-coral px-4 text-xs
                    font-bold text-[var(--text-on-accent)]"
                >
                  {a.reject}
                </button>
                {/* 0306：暂缓口（证据不足挂起），后端 v4_http 早已就绪 */}
                <button
                  onClick={() => onPostpone(t.id)}
                  className="ux-tap rounded-full border border-line px-3 text-xs
                    font-bold text-sub"
                >
                  {a.postpone}
                </button>
              </>
            )}
            </div>
          </li>
        ))}
        {failed && (
          <li className="py-6 text-center" role="alert">
            <span className="text-[var(--danger-text)]">
              {dict.review.loadFailedRetry}
            </span>
            <button
              type="button"
              className="ml-3 text-link underline"
              onClick={onRetry}
            >
              {dict.common.retry}
            </button>
          </li>
        )}
        {!failed && reviews.length === 0 && (
          <li className="py-6 text-center text-sub">
            {postponed ? a.queuePostponedEmpty : a.queueEmpty}
          </li>
        )}
      </ul>
    </section>
  );
}
