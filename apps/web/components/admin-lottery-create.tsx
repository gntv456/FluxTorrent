"use client";

/** 固定抽签运营面板·创建表单半场（0520 拆件守 300 行门禁）。
 *  奖项动态行 + 触发方式切换 + POST 创建草稿。
 *  主面板（列表/开奖/履约）在 admin-fixed-lottery.tsx。 */

import { useState } from "react";
import { BTN_SM_GHOST, BTN_SM_SKY, INPUT_CARD, PANEL_MD }
  from "@/lib/ui-classes";
import { api } from "@/lib/api-client";
import type { useI18n } from "@/i18n/client";

type PrizeDraft = {
  name: string;
  quantity: number;
  reward_kind: string;
  reward_config: string;
};

const KINDS = [
  "spark", "invite", "item", "medal", "upload",
  "vip", "ad_free", "rainbow_id", "rename_card", "custom",
];

/** props.act 由主面板传入（同一错误/成功提示通道）。 */
export function AdminLotteryCreate({
  t,
  act,
}: {
  t: ReturnType<typeof useI18n>["dict"]["adminLottery"];
  act: (fn: () => Promise<unknown>, ok: string) => Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [intro, setIntro] = useState("");
  const [trig, setTrig] = useState<
    "scheduled" | "threshold">("scheduled");
  const [drawAt, setDrawAt] = useState("");
  const [threshold, setThreshold] = useState("");
  const [fallback, setFallback] = useState("");
  const [price, setPrice] = useState("0");
  const [claimDays, setClaimDays] = useState("14");
  const [prizes, setPrizes] = useState<PrizeDraft[]>([{
    name: "", quantity: 1, reward_kind: "spark",
    reward_config: '{"amount": 100}',
  }]);

  async function create() {
    const body = {
      title,
      intro,
      trigger_type: trig,
      scheduled_draw_at:
        trig === "scheduled" && drawAt
          ? new Date(drawAt).toISOString() : undefined,
      threshold_count:
        trig === "threshold" && threshold
          ? Number(threshold) : undefined,
      fallback_draw_at:
        trig === "threshold" && fallback
          ? new Date(fallback).toISOString() : undefined,
      ticket_price: Number(price) || 0,
      claim_default_days: Number(claimDays) || 14,
      eligibility: {},
      prizes: prizes.map((p) => {
        let cfg: Record<string, unknown> = {};
        try {
          cfg = p.reward_config.trim()
            ? JSON.parse(p.reward_config) : {};
        } catch {
          cfg = {};
        }
        return {
          name: p.name,
          quantity: p.quantity,
          reward_kind: p.reward_kind,
          reward_config: cfg,
        };
      }),
    };
    await act(
      () => api.post("/api/v1/admin/lottery/campaigns", body),
      t.saved
    );
  }

  return (
      <section className={PANEL_MD}>
        <h2 className="mb-2 font-bold">{t.createTitle}</h2>
        <div className="grid grid-cols-2 gap-2">
          <input className={INPUT_CARD} placeholder={t.fTitle}
            value={title} onChange={(e) => setTitle(e.target.value)} />
          <input className={INPUT_CARD} placeholder={t.fIntro}
            value={intro} onChange={(e) => setIntro(e.target.value)} />
          <select className={INPUT_CARD}
            value={trig}
            onChange={(e) => setTrig(
              e.target.value as "scheduled" | "threshold"
            )}>
            <option value="scheduled">{t.trigScheduled}</option>
            <option value="threshold">{t.trigThreshold}</option>
          </select>
          {trig === "scheduled" ? (
            <input className={INPUT_CARD} type="datetime-local"
              placeholder={t.fDrawAt} value={drawAt}
              onChange={(e) => setDrawAt(e.target.value)} />
          ) : (
            <>
              <input className={INPUT_CARD} type="number"
                placeholder={t.fThreshold} value={threshold}
                onChange={(e) => setThreshold(e.target.value)} />
              <input className={INPUT_CARD} type="datetime-local"
                placeholder={t.fFallback} value={fallback}
                onChange={(e) => setFallback(e.target.value)} />
            </>
          )}
          <input className={INPUT_CARD} type="number"
            placeholder={t.fPrice} value={price}
            onChange={(e) => setPrice(e.target.value)} />
          <input className={INPUT_CARD} type="number"
            placeholder={t.fClaimDays} value={claimDays}
            onChange={(e) => setClaimDays(e.target.value)} />
        </div>
        <h3 className="mb-1 mt-3 text-sm font-bold">{t.fPrizes}</h3>
        {prizes.map((p, i) => (
          <div key={i} className="mb-2 grid grid-cols-4 gap-2">
            <input className={INPUT_CARD} placeholder={t.fName}
              value={p.name}
              onChange={(e) => setPrizes(prizes.map((x, j) =>
                j === i ? { ...x, name: e.target.value } : x))} />
            <input className={INPUT_CARD} type="number" min={1}
              max={1000} placeholder={t.fQty} value={p.quantity}
              onChange={(e) => setPrizes(prizes.map((x, j) =>
                j === i
                  ? { ...x, quantity: Number(e.target.value) || 1 }
                  : x))} />
            <select className={INPUT_CARD} value={p.reward_kind}
              onChange={(e) => setPrizes(prizes.map((x, j) =>
                j === i ? { ...x, reward_kind: e.target.value } : x))}>
              {KINDS.map((k) => (
                <option key={k} value={k}>{k}</option>
              ))}
            </select>
            <input className={INPUT_CARD} placeholder={t.fConfig}
              value={p.reward_config}
              onChange={(e) => setPrizes(prizes.map((x, j) =>
                j === i ? { ...x, reward_config: e.target.value } : x))} />
          </div>
        ))}
        <div className="mt-2 flex gap-2">
          <button className={BTN_SM_GHOST}
            onClick={() => prizes.length < 5 &&
              setPrizes([...prizes, {
                name: "", quantity: 1, reward_kind: "spark",
                reward_config: '{"amount": 100}',
              }])}>
            {t.addPrize}
          </button>
          <button className={BTN_SM_SKY} disabled={!title.trim()}
            onClick={() => void create()}>
            {t.create}
          </button>
        </div>
      </section>
  );
}
