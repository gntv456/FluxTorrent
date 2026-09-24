"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/**
 * 购买按钮（§8.2：写操作带幂等键；错误按 code 映射文案）。
 * 改版新增「余额不足」态：`affordable=false` 时按钮禁用并显示差额（由 RSC 侧
 * 用 /me 的 spark_balance 预判），把无效点击与报错挡在点击之前。
 */
export function BuyButton({
  itemId,
  affordable = true,
  shortBy = null,
}: {
  itemId: number;
  /** 余额是否足够（余额未知时传 true，交给后端校验） */
  affordable?: boolean;
  /** 差额文案（如「还差 20,000」）；缺省不显示 */
  shortBy?: string | null;
}) {
  const { dict } = useI18n();
  const [state, setState] = useState<"idle" | "busy" | "done">("idle");
  const [message, setMessage] = useState<string | null>(null);
  // 幂等键：挂载时生成一次，重试复用（§8.2 幂等语义），成功后不再购买
  const [idem] = useState(() => `web-${itemId}-${crypto.randomUUID()}`);

  async function buy() {
    setState("busy");
    setMessage(null);
    try {
      await api.post("/api/v1/shop/buy", {
        item_id: itemId,
        idempotency_key: idem,
      });
      setState("done");
      setMessage(dict.shop.buyOk);
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
        onClick={buy}
        disabled={state !== "idle" || !affordable}
        className={`btn btn-sm${affordable ? " btn-primary" : ""}`}
      >
        {label}
      </button>
      {message && (
        <p role="status" className="text-xs text-sub">
          {message}
        </p>
      )}
    </div>
  );
}
