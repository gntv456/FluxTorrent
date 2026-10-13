"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { timeAgo } from "@/lib/time-ago";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { toast } from "@/components/ui/toast";

/** 样式常量（抽出来避免 JSX 内联 className 行超 80 列门禁） */
const CARD =
  "rounded-[var(--r-md)] border border-line " +
  "bg-[var(--surface-card)] p-3";
const FULFILL_BTN =
  "min-h-[28px] rounded-full border border-line px-2 " +
  "text-[11px] font-bold text-sky-deep";
const ROW = "flex justify-between gap-2";

export interface RequestCardRow {
  id: number;
  username: string | null;
  title: string;
  descr: string | null;
  bounty: number;
  latest_bounty: number | null;
  comments: number | null;
  bids: number | null;
  fulfilled_torrent_id: number | null;
  created_at: string;
}

/** 求种卡片视图（窄屏 < md 渲染）。
 *
 * ZT81（2026-10-02）：/requests 桌面端是 8 列 nowrap 表格（首列 min-width 260px），
 * 390px 屏必须左右拖拽才能看全一行，且列头与数据错位。这里复用站内既有的
 * 「表格 → 卡片」降级范式（见 torrent-row-card.tsx），窄屏改纵向卡片。
 */
export function RequestCardList({
  rows,
  onChanged,
}: {
  rows: RequestCardRow[];
  /** 应求成功后通知父级刷新列表 */
  onChanged: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.requests;
  // A1 反馈协议：应求（prompt/alert）→ ConfirmDialog + toast
  const [askId, setAskId] = useState<number | null>(null);
  const [tid, setTid] = useState("");

  function submit() {
    const id = askId;
    if (id == null) return;
    const n = parseInt(tid, 10);
    if (!Number.isFinite(n) || n <= 0) {
      toast.warn(t.fulfillInvalid);
      return;
    }
    api
      .post("/api/v1/requests/fulfill", {
        request_id: id,
        torrent_id: n,
      })
      .then(() => {
        toast.success(t.fulfillOk);
        setAskId(null);
        setTid("");
        onChanged();
      })
      .catch((e: unknown) => {
        toast.error(
          t.fulfillFailed + (e instanceof Error ? e.message : ""),
          {
            requestId: e instanceof ApiError ? e.requestId : undefined,
          },
        );
      });
  }

  return (
    <ul className="flex flex-col gap-2">
      {rows.map((r) => (
        <li key={r.id} className={CARD}>
          <span className="block text-sm font-bold">{r.title}</span>
          {r.descr && (
            <p className="mt-1 line-clamp-2 text-xs text-sub">{r.descr}</p>
          )}
          <dl className="mt-2 grid grid-cols-2 gap-x-3 gap-y-1 text-xs">
            <div className={ROW}>
              <dt className="text-sub">{t.colLatestBounty}</dt>
              <dd className="num font-bold">
                {r.latest_bounty?.toLocaleString() ?? "—"}
              </dd>
            </div>
            <div className={ROW}>
              <dt className="text-sub">{t.colBounty}</dt>
              <dd className="num">{r.bounty.toLocaleString()}</dd>
            </div>
            <div className={ROW}>
              <dt className="text-sub">{t.colComments}</dt>
              <dd className="num">{r.comments ?? "—"}</dd>
            </div>
            <div className={ROW}>
              <dt className="text-sub">{t.colBids}</dt>
              <dd className="num">{r.bids ?? "—"}</dd>
            </div>
          </dl>
          <div className="mt-2 flex items-center justify-between gap-2">
            <span className="truncate text-xs text-sub">
              {r.username ?? "—"} · {timeAgo(r.created_at, dict.common)}
            </span>
            {r.fulfilled_torrent_id ? (
              <a
                className="request-status is-done"
                href={`/torrent/${r.fulfilled_torrent_id}`}
              >
                {t.fulfilled}
              </a>
            ) : (
              <span className="flex items-center gap-1">
                <span className="request-status is-progress">
                  {t.pending}
                </span>
                <button
                  type="button"
                  className={FULFILL_BTN}
                  onClick={() => setAskId(r.id)}
                >
                  {t.fulfillBtn}
                </button>
              </span>
            )}
          </div>
        </li>
      ))}
      <ConfirmDialog
        open={askId !== null}
        onClose={() => setAskId(null)}
        onConfirm={submit}
        title={t.fulfillBtn}
        confirmLabel={t.fulfillBtn}
      >
        <label className="mt-2 flex flex-col gap-1 text-xs text-sub">
          {t.fulfillTorrentId}
          <input
            value={tid}
            onChange={(e) => setTid(e.target.value.replace(/\D/g, ""))}
            inputMode="numeric"
            placeholder="#"
            autoFocus
            className="min-h-[36px] rounded-[var(--r-sm)] border border-line
              bg-[var(--surface-card)] px-3 text-sm text-ink"
          />
        </label>
      </ConfirmDialog>
    </ul>
  );
}
