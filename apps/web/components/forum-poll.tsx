"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

export interface PollData {
  options: string[];
  closed: boolean;
  my_vote: number | null;
  total: number;
  counts: { index: number; votes: number }[];
}

/**
 * 主题页投票挂件（0125）：
 * 未投 + 未截止 → 选项按钮；已投/已截止 → 结果条（我的票高亮）。
 * 结果公开（未投也可见计数），与论坛「投票结果是公共讨论一部分」的口径一致。
 */
export function PollWidget({
  topicId,
  poll,
  canClose,
}: {
  topicId: number;
  poll: PollData;
  canClose: boolean;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const voted = poll.my_vote !== null && poll.my_vote !== undefined;
  const counts = new Map(poll.counts.map((c) => [c.index, c.votes]));
  const total = poll.total;

  async function vote(idx: number) {
    if (busy || voted || poll.closed) return;
    setBusy(true);
    setErr(null);
    try {
      await api.post("/api/v1/forums/poll/vote", { topic_id: topicId, option_index: idx });
      router.refresh();
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : dict.common.networkError);
      setBusy(false);
    }
  }

  async function close() {
    if (busy || !window.confirm(dict.forums.pollCloseConfirm)) return;
    setBusy(true);
    try {
      await api.post("/api/v1/forums/poll/close", { topic_id: topicId });
      router.refresh();
    } catch {
      setBusy(false);
    }
  }

  return (
    <div className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-bold text-ink">
          📊 {dict.forums.pollTitle}
          {poll.closed && (
            <span className="ml-2 rounded-full bg-[var(--surface-sunken)] px-2 py-0.5 text-[11px] text-sub">
              {dict.forums.pollClosed}
            </span>
          )}
        </span>
        <span className="num text-xs text-sub">
          {dict.forums.pollTotal.replace("{n}", String(total))}
        </span>
      </div>
      <div className="flex flex-col gap-1.5">
        {poll.options.map((opt, idx) => {
          const n = counts.get(idx) ?? 0;
          const pct = total > 0 ? Math.round((n / total) * 100) : 0;
          const mine = poll.my_vote === idx;
          if (!voted && !poll.closed) {
            return (
              <button
                key={idx}
                type="button"
                onClick={() => vote(idx)}
                disabled={busy}
                className="min-h-[38px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-left text-sm transition hover:border-sky hover:text-sky disabled:opacity-50"
              >
                {opt}
              </button>
            );
          }
          return (
            <div key={idx} className="relative overflow-hidden rounded-[var(--r-sm)] border border-line">
              <div
                className={`absolute inset-y-0 left-0 ${mine ? "bg-[var(--sky-soft)]" : "bg-[var(--surface-sunken)]"}`}
                style={{ width: `${pct}%` }}
                aria-hidden
              />
              <div className="relative flex items-center justify-between px-3 py-2 text-sm">
                <span className={mine ? "font-bold text-sky" : "text-ink"}>
                  {mine && "✓ "}
                  {opt}
                </span>
                <span className="num text-xs text-sub">
                  {n} · {pct}%
                </span>
              </div>
            </div>
          );
        })}
      </div>
      {err && <p role="alert" className="mt-2 text-xs text-danger">{err}</p>}
      {canClose && !poll.closed && (
        <button
          type="button"
          onClick={close}
          disabled={busy}
          className="mt-2 rounded-full border border-line px-3 py-1 text-xs font-bold text-sub hover:border-coral hover:text-coral disabled:opacity-50"
        >
          {dict.forums.pollClose}
        </button>
      )}
    </div>
  );
}
