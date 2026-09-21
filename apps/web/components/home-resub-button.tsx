"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 首页签到与补签部件（从 home-sections.tsx 按域拆出，300 门禁）：
 *  ResubButton 补签卡弹层。签到日历本体在 home-sections-attendance.tsx。 */

/** 补签卡按钮（0066 修复）：持有数徽标 + 过去 7 天日期选择弹层 + 明确错误提示 */
export function ResubButton({ cards }: { cards: number }) {
  const { dict, currency } = useI18n();
  const t = dict.attResub2 ?? ({ btn: "补签" } as { btn?: string });
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  // 可补日期：过去 7 天内（YYYY-MM-DD），按近到远
  const days = Array.from({ length: 7 }, (_, i) => {
    const d = new Date(Date.now() - (i + 1) * 86400e3);
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
          idempotency_key: `resub-web-${date}-${crypto.randomUUID()}`,
        },
      );
      setMsg(
        `${dict.attResub2?.ok ?? "补签成功"}（剩余补签卡 ${r.cards_left}）`,
      );
      setTimeout(() => window.location.reload(), 800);
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "补签失败");
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <button
        type="button"
        className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold text-sub disabled:opacity-50"
        title={
          dict.attResub2?.note ??
          "消耗补签卡（商店购买或管理发放），补过去 7 天内漏签"
        }
        disabled={busy}
        onClick={() => setOpen(!open)}
      >
        {t.btn ?? "补签"}
        <span
          className={`ml-1 rounded-full px-1.5 text-[10px] ${cards > 0 ? "bg-mint/30 text-ink" : "bg-coral/20 text-danger"}`}
        >
          ×{cards}
        </span>
      </button>
      {open && (
        <div
          className="fixed inset-0 z-30 flex items-center justify-center bg-black/30 p-3"
          role="dialog"
          aria-modal="true"
          onClick={() => setOpen(false)}
        >
          <div
            className="w-full max-w-sm rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="mb-1 text-sm font-bold">
              {t.btn ?? "补签"}（持有 {cards} 张）
            </h3>
            <p className="mb-3 text-xs text-sub">
              {dict.attResub2?.note ?? "消耗补签卡，补过去 7 天内漏签"}
            </p>
            {cards === 0 ? (
              <p className="rounded-[var(--r-md)] bg-sun/20 p-3 text-xs text-ink">
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
                    disabled={busy}
                    onClick={() => doResub(d)}
                    className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold disabled:opacity-50 hover:border-sky"
                  >
                    {d.slice(5)}
                  </button>
                ))}
              </div>
            )}
            {msg && (
              <p className="mt-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink">
                {msg}
              </p>
            )}
            <button
              className="mt-3 min-h-[32px] rounded-full border border-line px-3 text-xs"
              onClick={() => setOpen(false)}
            >
              关闭
            </button>
          </div>
        </div>
      )}
    </>
  );
}
