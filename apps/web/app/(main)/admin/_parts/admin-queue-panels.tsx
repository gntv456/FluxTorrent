"use client";

import { PANEL_LG } from "@/lib/ui-classes";

/**
 * 管理后台待办队列面板（从 app/(main)/admin/page.tsx 按域拆出）：
 * 待审种子审核（ReviewsPanel）与申诉处理（AppealsPanel）。
 * 判定动作（approve/reject/handle）通过回调交还管理页。
 */

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { AppealRow, PendingTorrent } from "./admin-shared";

export function ReviewsPanel({
  reviews,
  onDecide,
}: {
  reviews: PendingTorrent[];
  onDecide: (torrentId: number, approve: boolean) => void;
}) {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  return (
    <section className={PANEL_LG}>
      <ul className="flex flex-col divide-y divide-line">
        {reviews.map((t) => (
          <li key={t.id} className="flex items-center gap-3 py-2">
            <div className="flex-1">
              <p className="text-sm font-bold">{t.name}</p>
              <p className="text-xs text-sub">
                #{t.id} ·{" "}
                {fmt(a.uploader, {
                  name: t.owner_id ?? dict.torrent.anonymous,
                })}{" "}
                · {(t.size / 1024 / 1024 / 1024).toFixed(2)}GB
              </p>
            </div>
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
