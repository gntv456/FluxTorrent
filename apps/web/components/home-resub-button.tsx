"use client";

import { useState } from "react";
import { Modal } from "@/components/modal";
import { api } from "@/lib/api-client";
import { fmt } from "@/i18n/config";
import { useI18n } from "@/i18n/client";
import { clientUuid } from "@/lib/client-uuid";

/** 首页签到与补签部件（从 home-sections.tsx 按域拆出，300 门禁）：
 *  ResubButton 补签卡弹层。签到日历本体在 home-sections-attendance.tsx。 */

/** 补签卡按钮（0066 修复）：持有数徽标 + 过去 7 天日期选择弹层 + 明确错误提示 */
export function ResubButton({
  cards,
  onCardsChange,
}: {
  cards: number;
  onCardsChange?: (n: number) => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.attResub2 ?? ({ btn: dict.review.resubMissed } as { btn?:
    string });
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [signed, setSigned] = useState<Set<string>>(new Set());
  // 可补日期：过去 30 天内（YYYY-MM-DD），按近到远——与后端 resub 窗口
  // （gaps_http/misc.rs 1..=30，2026-11-15 工单 8221 对齐旧站 30 天口径）。
  // 按站点时区（UTC+8）取日：此前直接 toISOString 切 UTC 日期——北京
  // 0-8 点窗口「昨天」不在列表、尾项落到第 8 天（后端必拒），与
  // home-attendance.tsx 的 todayStr 口径对齐。
  const days = Array.from({ length: 30 }, (_, i) => {
    const d = new Date(Date.now() + 8 * 3600e3 - (i + 1) * 86400e3);
    return d.toISOString().slice(0, 10);
  });
  async function doResub(date: string) {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ cards_left: number }>(
        "/api/v1/attendance/resub",
        {
          target_date: date,
          idempotency_key: `resub-web-${date}-${clientUuid()}`,
        },
      );
      setMsg(dict.review.resubDone);
      // 本地标记该日已补签，并把卡数回传给父层；不再 800ms 整页 reload
      // （会把提示和滚动位置一起冲掉）。
      setSigned((prev) => new Set(prev).add(date));
      onCardsChange?.(r.cards_left);
    } catch (e) {
      setMsg(dict.review.resubFail);
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <button
        type="button"
        className="min-h-11 rounded-full border border-line px-3 text-xs
          font-bold
          text-sub disabled:opacity-50"
        title={dict.attResub2.note}
        disabled={busy}
        onClick={() => setOpen(!open)}
      >
        {t.btn}
        <span
          className={`ml-1 rounded-full px-1.5 text-xs ${cards > 0 ?
            "bg-mint/30 text-ink"
            : "bg-coral/20 text-[var(--danger-text)]"}`}
        >
          ×{cards}
        </span>
      </button>
      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={t.btn + fmt(t.holdSuffix, { n: cards })}
      >
        <div className="flex flex-col">
            {cards === 0 ? (
              <p className="rounded-[var(--r-md)] bg-sun/20 p-3 text-xs
                text-ink">
                暂无可用补签卡：可到
                <a href="/shop" className="font-bold text-sky">
                  {currency}商店
                </a>
                购买，或等管理发放。
              </p>
            ) : (
              <div className="flex flex-wrap gap-2">
                {days.map((d) => (
                  <button
                    key={d}
                    disabled={busy || signed.has(d)}
                    onClick={() => doResub(d)}
                    className={`min-h-11 rounded-full border px-3 text-xs
                      font-bold
                      disabled:opacity-50 ${
                      signed.has(d)
                        ? "border-mint bg-mint/20 text-mint"
                        : "border-line hover:border-sky"
                    }`}
                    title={signed.has(d) ? dict.review.resubSigned : undefined}
                  >
                    {d.slice(5)}
                    {signed.has(d) ? " ✓" : ""}
                  </button>
                ))}
              </div>
            )}
            {msg && (
              <p className="mt-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs
                text-ink">
                {msg}
              </p>
            )}
        </div>
      </Modal>
    </>
  );
}
