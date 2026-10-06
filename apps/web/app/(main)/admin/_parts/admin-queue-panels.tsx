"use client";

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

/** 待审时长（0286）：旧版取了 created_at 却从不渲染，压了三天看上去还是三天。 */
function waitingText(iso: string): string {
  const mins = Math.max(
    0,
    Math.floor((Date.now() - new Date(iso).getTime()) / 60000),
  );
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 48) return `${hrs}h`;
  return `${Math.floor(hrs / 24)}d`;
}

export function ReviewsPanel({
  reviews,
  total,
  offset,
  onPage,
  denyReasons,
  denyReasonId,
  onDenyReason,
  onDecide,
}: {
  reviews: PendingTorrent[];
  total: number;
  offset: number;
  onPage: (next: number) => void;
  denyReasons: DenyReason[];
  denyReasonId: number | null;
  onDenyReason: (id: number | null) => void;
  onDecide: (torrentId: number, approve: boolean) => void;
}) {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const pageSize = reviews.length || 50;
  return (
    <section className={PANEL_LG}>
      <div className="mb-2 flex flex-wrap items-center gap-2 text-xs">
        <span className="text-sub">{fmt(a.queueTotal, { n: total })}</span>
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
        <span className="ml-auto flex gap-1">
          <button
            disabled={offset <= 0}
            onClick={() => onPage(Math.max(0, offset - pageSize))}
            className="rounded border border-line px-2 py-1 disabled:opacity-40"
          >
            {a.queuePrev}
          </button>
          <button
            disabled={offset + reviews.length >= total}
            onClick={() => onPage(offset + pageSize)}
            className="rounded border border-line px-2 py-1 disabled:opacity-40"
          >
            {a.queueNext}
          </button>
        </span>
      </div>
      <ul className="flex flex-col divide-y divide-line">
        {reviews.map((t) => (
          <li key={t.id} className="flex items-start gap-3 py-2">
            <div className="flex-1">
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
              {t.descr_excerpt ? (
                <p className="mt-1 line-clamp-3 text-xs text-sub">
                  {t.descr_excerpt.replace(/<[^>]+>/g, "")}
                </p>
              ) : null}
            </div>
            <a
              href={`/torrent/${t.id}`}
              target="_blank"
              rel="noreferrer"
              className="self-center text-xs underline text-sub"
            >
              {a.queueOpen}
            </a>
            <button
              onClick={() => onDecide(t.id, true)}
              className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white"
            >
              {a.approve}
            </button>
            <button
              onClick={() => onDecide(t.id, false)}
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
}

export function AppealsPanel({
  appeals,
  onHandle,
}: {
  appeals: AppealRow[];
  onHandle: (id: number, accept: boolean) => void;
}) {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  // 申诉类型四值（后端 appeals.rs CHECK），与用户侧 /appeals 同一组文案
  const KIND: Record<string, string> = {
    ban: dict.appeals.kindBan,
    hr: dict.appeals.kindHr,
    warn: dict.appeals.kindWarn,
    other: dict.appeals.kindOther,
  };
  return (
    <section className={PANEL_LG}>
      <ul className="flex flex-col divide-y divide-line">
        {appeals.map((ap) => (
          <li key={ap.id} className="flex items-center gap-3 py-2">
            <div className="flex-1">
              <p className="text-sm">
                <span className="rounded-full bg-sun/30 px-2 py-0.5 text-[10px]">
                  {KIND[ap.kind] ?? ap.kind}
                </span>{" "}
                <b>{ap.username}</b>
                {ap.ref_id !== null && (
                  <span className="text-xs text-sub"> · #{ap.ref_id}</span>
                )}
                {ap.status !== "open" && (
                  <span
                    className={`ml-1 rounded-full px-2 py-0.5 text-[10px] ${
                      ap.status === "accepted"
                        ? "bg-mint/30"
                        : "bg-coral/20 text-danger"
                    }`}
                  >
                    {ap.status === "accepted"
                      ? a.appealAccepted
                      : a.appealRejected}
                  </span>
                )}
              </p>
              <p className="text-xs text-sub">
                {ap.body}
                {ap.result_note
                  ? ` · ${a.appealNoteLabel}: ${ap.result_note}`
                  : ""}
              </p>
            </div>
            {ap.status === "open" && (
              <span className="flex gap-2">
                <button
                  onClick={() => onHandle(ap.id, true)}
                  className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white"
                >
                  {a.appealAccept}
                </button>
                <button
                  onClick={() => onHandle(ap.id, false)}
                  className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger"
                >
                  {a.appealReject}
                </button>
              </span>
            )}
          </li>
        ))}
        {appeals.length === 0 && (
          <li className="py-6 text-center text-sub">{a.appealEmpty}</li>
        )}
      </ul>
    </section>
  );
}
