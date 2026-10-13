"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { clientUuid } from "@/lib/client-uuid";
import { toast } from "@/components/ui/toast";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";

/**
 * 购买按钮（§8.2：写操作带幂等键；错误按 code 映射文案）。
 * 「余额不足」态：`affordable=false` 时按钮禁用并显示差额（RSC 侧用 /me 的
 * spark_balance 预判），把无效点击与报错挡在点击之前。
 * 0207：点开改为覆盖式弹窗——可叠加类（config.stackable）可选数量 1-100，
 * 一口价/装扮类固定 1 件；提交前展示总价预判（余额不足时按总额禁用）。
 * W-18 表单协议试点：确认弹窗换 ConfirmDialog（焦点陷阱/Esc/遮罩语义
 * 统一），购买失败走 toast.error + request_id（余额不足 4001 等可诊断）。
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
  const [busy, setBusy] = useState(false);
  const [bought, setBought] = useState(false);
  // 幂等键：每次打开弹窗生成一次，弹窗内重试复用（§8.2 幂等语义）；
  // 购买成功后换新键（0207b：连买两单不再被幂等挡）
  const [idem, setIdem] = useState(() => `web-${itemId}-${clientUuid()}`);

  const total = unitPrice * qty;
  const totalOk = balance === null || balance >= total;

  async function buy() {
    setBusy(true);
    try {
      await api.post("/api/v1/shop/buy", {
        item_id: itemId,
        qty: stackable ? qty : 1,
        idempotency_key: idem,
      });
      // 成功反馈落在卡片按钮「已购买 ✓」+ 父层刷新的余额上（0207b）；
      // 换新幂等键，连买第二单不被上一单的键挡住
      setBought(true);
      setOpen(false);
      setIdem(`web-${itemId}-${clientUuid()}`);
      onBought?.();
    } catch (err) {
      // W-18：购买失败（余额不足 4001 等）走全局 toast——错误需要用户
      // 记住且可报障（request_id 芯片一键复制），行内 msg 承担不了
      toast.error(apiErrorMessage(dict, err), {
        requestId: err instanceof ApiError ? err.requestId : undefined,
      });
    } finally {
      setBusy(false);
    }
  }

  const label = busy
    ? dict.shop.buying
    : bought
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
        disabled={busy || bought || !affordable}
        className={`btn btn-sm${affordable ? " btn-primary" : ""}`}
      >
        {label}
      </button>
      <ConfirmDialog
        open={open}
        onClose={() => {
          if (!busy) setOpen(false);
        }}
        onConfirm={() => void buy()}
        title={t.title}
        message={
          stackable
            ? `${t.noRefund}（${qty} × ${unitPrice.toLocaleString()} = ${
                total.toLocaleString()
              } ${currency}）`
            : `${t.fixedOne}（${total.toLocaleString()} ${currency}）`
        }
        busy={busy}
        confirmLabel={totalOk ? t.confirm : dict.shop.insufficient}
      >
        {/* 数量选择（可叠加类）：ConfirmDialog 消息区下方的内容槽 */}
        {stackable ? (
          <label
            className="mt-2 flex items-center justify-between
              gap-3 text-sm"
          >
            <span className="text-sub">{t.qty}</span>
            <span className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => setQty((q) => Math.max(1, q - 1))}
                disabled={busy || qty <= 1}
                className="h-8 w-8 rounded-full border border-line text-sm
                  font-bold disabled:opacity-40"
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
                disabled={busy}
                className="h-8 w-16 rounded-[var(--r-sm)] border
                  border-line bg-transparent text-center text-sm"
                aria-label={t.qty}
              />
              <button
                type="button"
                onClick={() => setQty((q) => Math.min(100, q + 1))}
                disabled={busy || qty >= 100}
                className="h-8 w-8 rounded-full border border-line text-sm
                  font-bold disabled:opacity-40"
              >
                +
              </button>
            </span>
          </label>
        ) : null}
        <p className="mt-2 flex items-baseline justify-between text-sm">
          <span className="text-sub">{t.total}</span>
          <b className={totalOk ? "" : "text-danger"}>
            {total.toLocaleString()}{" "}
            <span className="text-xs font-normal text-sub">{currency}</span>
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
      </ConfirmDialog>
    </div>
  );
}
