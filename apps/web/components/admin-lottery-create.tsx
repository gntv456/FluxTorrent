"use client";

/** 固定抽签运营面板·创建表单半场（0520 拆件；0523 人性化重写）。
 *  奖项动态行：奖励类型走本地语言候选；配置字段按类型切换结构化
 *  输入（数值框带标签/范围/缺省），不再手写 JSON。主面板在
 *  admin-fixed-lottery.tsx；字段规格在 admin-lottery-shared.ts。 */

import { useState } from "react";
import { BTN_SM_GHOST, BTN_SM_SKY, INPUT_CARD, PANEL_MD }
  from "@/lib/ui-classes";
import { api } from "@/lib/api-client";
import {
  LOTTERY_KINDS,
  buildConfig,
  kindMeta,
  type CfgField,
  type LotteryDict,
} from "./admin-lottery-shared";

type PrizeDraft = {
  name: string;
  quantity: number;
  reward_kind: string;
  cfgValues: Record<string, number | string>;
};

const newPrize = (): PrizeDraft => ({
  name: "",
  quantity: 1,
  reward_kind: "spark",
  cfgValues: { amount: 100 },
});

/** 切换类型时按字段规格重建配置值（保留同名旧值）。 */
function switchKind(p: PrizeDraft, kind: string): PrizeDraft {
  const meta = kindMeta(kind);
  const cfgValues: Record<string, number | string> = {};
  for (const f of meta.fields) {
    cfgValues[f.key] =
      p.cfgValues[f.key] !== undefined ? p.cfgValues[f.key] : f.defaultValue;
  }
  return { ...p, reward_kind: kind, cfgValues };
}

/** 单字段输入：数值框 min/标签/缺省提示，文本框 plain。 */
function CfgInput({
  f, value, onChange, t,
}: {
  f: CfgField;
  value: number | string | undefined;
  onChange: (v: number | string) => void;
  t: LotteryDict;
}) {
  return (
    <label className="flex min-w-0 flex-col gap-0.5">
      <span className="text-xs opacity-70">
        {t[f.labelKey]}
        {f.hintKey ? (
          <span className="ml-1 opacity-60">({t[f.hintKey]})</span>
        ) : null}
      </span>
      <input
        className={INPUT_CARD}
        type={f.kind === "int" ? "number" : "text"}
        min={f.min}
        value={value ?? f.defaultValue}
        onChange={(e) =>
          onChange(f.kind === "int"
            ? Number(e.target.value)
            : e.target.value)}
      />
    </label>
  );
}

/** props.act 由主面板传入（同一错误/成功提示通道）。 */
export function AdminLotteryCreate({
  t,
  act,
}: {
  t: LotteryDict;
  act: (fn: () => Promise<unknown>, ok: string) => Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [intro, setIntro] = useState("");
  const [trig, setTrig] = useState<"scheduled" | "threshold">("scheduled");
  const [drawAt, setDrawAt] = useState("");
  const [threshold, setThreshold] = useState(50);
  const [fallback, setFallback] = useState("");
  const [price, setPrice] = useState(0);
  const [claimDays, setClaimDays] = useState(14);
  const [prizes, setPrizes] = useState<PrizeDraft[]>([newPrize()]);

  async function create() {
    const body = {
      title,
      intro,
      trigger_type: trig,
      scheduled_draw_at:
        trig === "scheduled" && drawAt
          ? new Date(drawAt).toISOString() : undefined,
      threshold_count: trig === "threshold" ? threshold : undefined,
      fallback_draw_at:
        trig === "threshold" && fallback
          ? new Date(fallback).toISOString() : undefined,
      ticket_price: price,
      claim_default_days: claimDays,
      eligibility: {},
      prizes: prizes.map((p) => ({
        name: p.name,
        quantity: p.quantity,
        reward_kind: p.reward_kind,
        reward_config: buildConfig(kindMeta(p.reward_kind).fields,
          p.cfgValues),
      })),
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
          <label className="flex flex-col gap-0.5">
            <span className="text-xs opacity-70">{t.fTitle}</span>
            <input className={INPUT_CARD} value={title}
              onChange={(e) => setTitle(e.target.value)} />
          </label>
          <label className="flex flex-col gap-0.5">
            <span className="text-xs opacity-70">{t.fIntro}</span>
            <input className={INPUT_CARD} value={intro}
              onChange={(e) => setIntro(e.target.value)} />
          </label>
          <label className="flex flex-col gap-0.5">
            <span className="text-xs opacity-70">{t.fTrigger}</span>
            <select className={INPUT_CARD} value={trig}
              onChange={(e) => setTrig(
                e.target.value as "scheduled" | "threshold"
              )}>
              <option value="scheduled">{t.trigScheduled}</option>
              <option value="threshold">{t.trigThreshold}</option>
            </select>
          </label>
          {trig === "scheduled" ? (
            <label className="flex flex-col gap-0.5">
              <span className="text-xs opacity-70">{t.fDrawAt}</span>
              <input className={INPUT_CARD} type="datetime-local"
                value={drawAt}
                onChange={(e) => setDrawAt(e.target.value)} />
            </label>
          ) : (
            <>
              <label className="flex flex-col gap-0.5">
                <span className="text-xs opacity-70">{t.fThreshold}</span>
                <input className={INPUT_CARD} type="number" min={1}
                  value={threshold}
                  onChange={(e) =>
                    setThreshold(Number(e.target.value) || 1)} />
              </label>
              <label className="flex flex-col gap-0.5">
                <span className="text-xs opacity-70">{t.fFallback}</span>
                <input className={INPUT_CARD} type="datetime-local"
                  value={fallback}
                  onChange={(e) => setFallback(e.target.value)} />
              </label>
            </>
          )}
          <label className="flex flex-col gap-0.5">
            <span className="text-xs opacity-70">{t.fPrice}</span>
            <input className={INPUT_CARD} type="number" min={0}
              value={price}
              onChange={(e) => setPrice(Number(e.target.value) || 0)} />
          </label>
          <label className="flex flex-col gap-0.5">
            <span className="text-xs opacity-70">{t.fClaimDays}</span>
            <input className={INPUT_CARD} type="number" min={1}
              value={claimDays}
              onChange={(e) =>
                setClaimDays(Number(e.target.value) || 14)} />
          </label>
        </div>
        <h3 className="mb-1 mt-3 text-sm font-bold">{t.fPrizes}</h3>
        {prizes.map((p, i) => {
          const meta = kindMeta(p.reward_kind);
          return (
            <div key={i} className="mb-2 flex flex-col gap-1
              rounded-lg border border-line/40 p-2">
              <div className="grid grid-cols-4 gap-2">
                <label className="flex flex-col gap-0.5">
                  <span className="text-xs opacity-70">{t.fName}</span>
                  <input className={INPUT_CARD} value={p.name}
                    onChange={(e) => setPrizes(prizes.map((x, j) =>
                      j === i ? { ...x, name: e.target.value } : x))} />
                </label>
                <label className="flex flex-col gap-0.5">
                  <span className="text-xs opacity-70">{t.fQty}</span>
                  <input className={INPUT_CARD} type="number" min={1}
                    max={1000} value={p.quantity}
                    onChange={(e) => setPrizes(prizes.map((x, j) =>
                      j === i
                        ? { ...x, quantity: Number(e.target.value) || 1 }
                        : x))} />
                </label>
                <label className="flex flex-col gap-0.5">
                  <span className="text-xs opacity-70">{t.fKind}</span>
                  <select className={INPUT_CARD}
                    value={p.reward_kind}
                    onChange={(e) => setPrizes(prizes.map((x, j) =>
                      j === i ? switchKind(x, e.target.value) : x))}>
                    {LOTTERY_KINDS.map((k) => (
                      <option key={k.kind} value={k.kind}>
                        {t[k.labelKey]}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="flex items-end pb-0.5">
                  <button type="button" className={BTN_SM_GHOST}
                    disabled={prizes.length <= 1}
                    onClick={() => setPrizes(
                      prizes.filter((_, j) => j !== i))}>
                    {t.delPrize}
                  </button>
                </div>
              </div>
              {meta.fields.length > 0 && (
                <div className="grid grid-cols-4 gap-2">
                  {meta.fields.map((f) => (
                    <CfgInput key={f.key} f={f} t={t}
                      value={p.cfgValues[f.key]}
                      onChange={(v) => setPrizes(prizes.map((x, j) =>
                        j === i ? {
                          ...x,
                          cfgValues: { ...x.cfgValues, [f.key]: v },
                        } : x))} />
                  ))}
                </div>
              )}
            </div>
          );
        })}
        <div className="mt-2 flex gap-2">
          <button type="button" className={BTN_SM_GHOST}
            disabled={prizes.length >= 5}
            onClick={() => setPrizes([...prizes, newPrize()])}>
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
