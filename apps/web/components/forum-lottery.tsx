"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

export interface LotteryData {
  winners: number;
  prize: number;
  ticket: number;
  status: string;
  draw_at: string;
  entries: number;
  joined: boolean;
  my_won: boolean;
  winner_ids: { id: number; name: string | null }[];
}

/**
 * 主题页抽奖挂件（0126）：
 * open → 「参与」按钮（票价 >0 显示票价）+ 参与人数 + 开奖倒计时；
 * drawn → 中奖名单（含我 ✓）；楼主/版主可提前开奖。
 */
export function LotteryWidget({
  topicId,
  lottery,
  currency,
  canDraw,
  isOp,
}: {
  topicId: number;
  lottery: LotteryData;
  currency: string;
  canDraw: boolean;
  isOp: boolean;
}) {
  const { dict, locale } = useI18n();
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  // 时间格式化放到挂载后（SSR 与容器的 ICU/时区可能不同，直接渲染会 418 水合不匹配）
  const [drawAtText, setDrawAtText] = useState("");
  useEffect(() => {
    setDrawAtText(new Date(lottery.draw_at).toLocaleString(dateLocale(locale)));
  }, [lottery.draw_at, locale]);

  async function join() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      await api.post("/api/v1/forums/lottery/join", { topic_id: topicId });
      router.refresh();
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : dict.common.networkError);
      setBusy(false);
    }
  }

  async function draw() {
    if (busy || !window.confirm(dict.forums.lotDrawConfirm)) return;
    setBusy(true);
    try {
      await api.post("/api/v1/forums/lottery/draw", { topic_id: topicId });
      router.refresh();
    } catch {
      setBusy(false);
    }
  }

  const open = lottery.status === "open";

  return (
    <div className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4">
      <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
        <span className="font-bold text-ink">
          🎁 {dict.forums.lotTitle}
          {!open && (
            <span className="ml-2 rounded-full bg-[var(--surface-sunken)] px-2 py-0.5 text-[11px] text-sub">
              {lottery.status === "drawn"
                ? dict.forums.lotDrawn
                : dict.forums.lotCancelled}
            </span>
          )}
        </span>
        <span className="text-xs text-sub">
          {dict.forums.lotPrizeLine
            .replace("{n}", String(lottery.winners))
            .replace("{p}", String(lottery.prize))
            .replace("{magic}", currency)}
        </span>
      </div>
      <div className="flex flex-wrap items-center gap-3 text-sm">
        <span className="num text-sub">
          {dict.forums.lotEntries.replace("{n}", String(lottery.entries))}
        </span>
        {open && (
          <span className="text-xs text-sub">
            {dict.forums.lotDrawAt} {drawAtText}
          </span>
        )}
      </div>
      {open && !isOp && (
        <button
          type="button"
          onClick={join}
          disabled={busy || lottery.joined}
          className={`mt-2 min-h-[38px] rounded-full px-4 text-sm font-bold transition disabled:opacity-60 ${
            lottery.joined
              ? "border border-line bg-[var(--sky-soft)] text-sky"
              : "bg-coral text-white active:scale-[0.97]"
          }`}
        >
          {lottery.joined
            ? dict.forums.lotJoined
            : lottery.ticket > 0
              ? `${dict.forums.lotJoin} · ${lottery.ticket} ${currency}`
              : dict.forums.lotJoinFree}
        </button>
      )}
      {open && canDraw && (
        <button
          type="button"
          onClick={draw}
          disabled={busy}
          className="ml-2 mt-2 min-h-[38px] rounded-full border border-coral px-4 text-sm font-bold text-coral transition hover:bg-[var(--coral-soft)] disabled:opacity-50"
        >
          {dict.forums.lotDrawNow}
        </button>
      )}
      {lottery.status === "drawn" && (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {lottery.winner_ids.length === 0 ? (
            <span className="text-xs text-sub">{dict.forums.lotNoEntries}</span>
          ) : (
            lottery.winner_ids.map((w) => (
              <span
                key={w.id}
                className="rounded-full bg-[var(--coral-soft)] px-2.5 py-0.5 text-xs font-bold text-coral"
              >
                🎉 {w.name ?? `#${w.id}`}
              </span>
            ))
          )}
        </div>
      )}
      {lottery.status === "drawn" && lottery.my_won && (
        <p className="mt-2 rounded-[var(--r-sm)] bg-[var(--coral-soft)] px-3 py-1.5 text-sm font-bold text-coral">
          🎉{" "}
          {dict.forums.lotYouWon
            .replace("{p}", String(lottery.prize))
            .replace("{magic}", currency)}
        </p>
      )}
      {err && (
        <p role="alert" className="mt-2 text-xs text-danger">
          {err}
        </p>
      )}
    </div>
  );
}
