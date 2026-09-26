"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

// 长类名常量（行宽 ≤80 门禁）：弹窗遮罩/卡片/关闭钮/数量行三件套
const MODAL_MASK =
  "fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4";
const MODAL_CARD =
  "w-full max-w-sm rounded-[var(--r-md)] border border-line " +
  "bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]";
const MODAL_CLOSE =
  "flex h-7 w-7 items-center justify-center rounded-full " +
  "border border-line text-xs text-sub";
const QTY_ROW = "flex items-center justify-between gap-3 text-sm";
const QTY_BTN =
  "h-8 w-8 rounded-full border border-line text-sm font-bold " +
  "disabled:opacity-40";
const QTY_INPUT =
  "h-8 w-16 rounded-[var(--r-sm)] border border-line " +
  "bg-transparent text-center text-sm";

/**
 * 购买按钮（§8.2：写操作带幂等键；错误按 code 映射文案）。
 * 「余额不足」态：`affordable=false` 时按钮禁用并显示差额（RSC 侧用 /me 的
 * spark_balance 预判），把无效点击与报错挡在点击之前。
 * 0207：点开改为覆盖式弹窗——可叠加类（config.stackable）可选数量 1-100，
 * 一口价/装扮类固定 1 件；提交前展示总价预判（余额不足时按总额禁用）。
 */
export function BuyButton({
  itemId,
  unitPrice,
  stackable = false,
  affordable = true,
  shortBy = null,
  balance,
  onBought,
}: {
  itemId: number;
  /** 单价（0207：按钮内按数量算总价，不再依赖外层预判单个价格） */
  unitPrice: number;
  /** 该商品可否叠加数量（后端 config.stackable 同口径） */
  stackable?: boolean;
  /** 余额是否足够买 1 件（余额未知时传 true，交给后端校验） */
  affordable?: boolean;
  /** 差额文案（如「还差 20,000」）；缺省不显示 */
  shortBy?: string | null;
  /** 当前余额（null=未知）：弹窗内按总价实时预判 */
  balance: number | null;
  /** 购买成功回调（0207b：父层刷新余额/拥有态） */
  onBought?: () => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.buyDialog;
  const [open, setOpen] = useState(false);
  const [qty, setQty] = useState(1);
  const [state, setState] = useState<"idle" | "busy" | "done">("idle");
  const [message, setMessage] = useState<string | null>(null);
  // 幂等键：每次打开弹窗生成一次，弹窗内重试复用（§8.2 幂等语义）；
  // 购买成功后换新键（0207b：连买两单不再被幂等挡）
  const [idem, setIdem] = useState(
    () => `web-${itemId}-${crypto.randomUUID()}`,
  );

  const total = unitPrice * qty;
  const totalOk = balance === null || balance >= total;

  async function buy() {
    setState("busy");
    setMessage(null);
    try {
      await api.post("/api/v1/shop/buy", {
        item_id: itemId,
        qty: stackable ? qty : 1,
        idempotency_key: idem,
      });
      setState("done");
      setMessage(dict.shop.buyOk);
      onBought?.();
      // 成功即收起弹窗（0207b）：反馈落在卡片按钮「已购买 ✓」+ 父层刷新的余额上；
      // 同时换新幂等键，连买第二单不被上一单的键挡住
      setOpen(false);
      setIdem(`web-${itemId}-${crypto.randomUUID()}`);
    } catch (err) {
      setState("idle");
      setMessage(apiErrorMessage(dict, err));
    }
  }

  const label =
    state === "busy"
      ? dict.shop.buying
      : state === "done"
        ? dict.shop.bought
        : affordable
          ? dict.shop.buy
          : dict.shop.insufficient;

  return (
    <div className="flex flex-col items-end gap-1">
      {!affordable && shortBy && (
        <span className="unit is-short">{shortBy}</span>
      )}
      <button
        type="button"
        onClick={() => setOpen(true)}
        disabled={state !== "idle" || !affordable}
        className={`btn btn-sm${affordable ? " btn-primary" : ""}`}
      >
        {label}
      </button>
      {message && !open && (
        <p role="status" className="text-xs text-sub">
          {message}
        </p>
      )}
      {open && (
        <div
          className={MODAL_MASK}
          onClick={() => state !== "busy" && setOpen(false)}
          role="presentation"
        >
          <div
            role="dialog"
            aria-modal
            aria-label={t.title}
            className={MODAL_CARD}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 flex items-center justify-between">
              <h3 className="text-sm font-bold">{t.title}</h3>
              <button
                type="button"
                onClick={() => state !== "busy" && setOpen(false)}
                aria-label="close"
                className={MODAL_CLOSE}
              >
                ×
              </button>
            </div>
            <div className="flex flex-col gap-3">
              {stackable ? (
                <label className={QTY_ROW}>
                  <span className="text-sub">{t.qty}</span>
                  <span className="flex items-center gap-1">
                    <button
                      type="button"
                      onClick={() => setQty((q) => Math.max(1, q - 1))}
                      disabled={state === "busy" || qty <= 1}
                      className={QTY_BTN}
                    >
                      −
                    </button>
                    <input
                      type="number"
                      min={1}
                      max={100}
                      value={qty}
                      onChange={(e) => {
                        const v = Number(e.target.value);
                        if (Number.isFinite(v)) {
                          setQty(Math.min(100, Math.max(1, Math.trunc(v))));
                        }
                      }}
                      disabled={state === "busy"}
                      className={QTY_INPUT}
                      aria-label={t.qty}
                    />
                    <button
                      type="button"
                      onClick={() => setQty((q) => Math.min(100, q + 1))}
                      disabled={state === "busy" || qty >= 100}
                      className={QTY_BTN}
                    >
                      +
                    </button>
                  </span>
                </label>
              ) : (
                <p className="text-xs text-sub">{t.fixedOne}</p>
              )}
              <p className="flex items-baseline justify-between text-sm">
                <span className="text-sub">{t.total}</span>
                <b className={totalOk ? "" : "text-danger"}>
                  {total.toLocaleString()}{" "}
                  <span className="text-xs font-normal text-sub">
                    {currency}
                  </span>
                </b>
              </p>
              {!totalOk && balance !== null && (
                <p className="text-xs text-danger">
                  {t.shortTotal.replace(
                    "{n}",
                    (total - balance).toLocaleString(),
                  )}
                </p>
              )}
              <div className="flex items-center justify-end gap-2">
                <button
                  type="button"
                  disabled={state === "busy"}
                  onClick={() => setOpen(false)}
                  className="btn btn-sm btn-ghost"
                >
                  {dict.common.cancel}
                </button>
                <button
                  type="button"
                  onClick={() => void buy()}
                  disabled={state !== "idle" || !totalOk}
                  className="btn btn-sm btn-primary"
                >
                  {state === "busy" ? dict.shop.buying : t.confirm}
                </button>
              </div>
              {message && (
                <p role="status" className="text-xs text-sub">
                  {message}
                </p>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
