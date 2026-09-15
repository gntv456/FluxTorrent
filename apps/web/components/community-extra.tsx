"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { OfferItem, PollItem } from "@/lib/data";

/** 趣味投票（参考站 funvote 口径：一人一票，投票 +1 火花） */
export function PollBox({ empty }: { empty: string }) {
  const { dict } = useI18n();
  const [polls, setPolls] = useState<PollItem[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  async function refresh() {
    try {
      setPolls(await api.get<PollItem[]>("/api/v1/fun/polls"));
    } catch {
      setPolls([]);
    }
  }
  useEffect(() => {
    refresh();
  }, []);

  async function vote(pollId: number, optionIndex: number) {
    setMsg(null);
    try {
      await api.post("/api/v1/fun/vote", { poll_id: pollId, option_index: optionIndex });
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    }
  }

  if (polls === null) return null;
  if (polls.length === 0) return <p className="py-4 text-center text-sub">{empty}</p>;
  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="text-sm text-sky-deep">{msg}</p>}
      {polls.map((p) => {
        const total = Math.max(1, p.total_votes);
        return (
          <div
            key={p.id}
            className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
          >
            <h3 className="font-bold">{p.question}</h3>
            <ul className="mt-2 flex flex-col gap-1.5">
              {p.options.map((opt, i) => {
                const votes = p.counts.find((c) => c.index === i)?.votes ?? 0;
                const pct = Math.round((votes / total) * 100);
                const mine = p.my_vote === i;
                return (
                  <li key={i}>
                    <button
                      type="button"
                      disabled={p.my_vote !== null}
                      onClick={() => vote(p.id, i)}
                      className={`w-full min-h-[36px] rounded-[var(--r-sm)] border px-3 text-left text-sm ${
                        mine ? "border-sky-deep bg-sky-soft" : "border-line bg-[var(--surface-card)]"
                      } ${p.my_vote !== null ? "cursor-default" : "hover:border-sky"}`}
                    >
                      <span className="flex items-center justify-between gap-2">
                        <span>
                          {mine ? "● " : "○ "}
                          {opt}
                        </span>
                        {p.my_vote !== null && (
                          <span className="num text-xs text-sub">
                            {votes} · {pct}%
                          </span>
                        )}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
            <p className="mt-1 text-xs text-sub num">
              {dict.polls.total}: {p.total_votes}
            </p>
          </div>
        );
      })}
    </div>
  );
}

/** 应求/候选列表（参考站 offers 口径：投票达标晋升为求种） */
export function OfferList({ offers, empty }: { offers: OfferItem[]; empty: string }) {
  if (offers.length === 0) return <p className="py-4 text-center text-sub">{empty}</p>;
  return (
    <ul className="flex flex-col gap-2">
      {offers.map((o) => (
        <li
          key={o.id}
          className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
        >
          <div className="min-w-0">
            <p className="truncate font-bold">{o.torrent_name ?? `#${o.torrent_id ?? "?"}`}</p>
            <p className="text-xs text-sub">{o.username ?? "—"}</p>
          </div>
          <span className="sticker bg-sun text-ink num">▲ {o.votes}</span>
        </li>
      ))}
    </ul>
  );
}
