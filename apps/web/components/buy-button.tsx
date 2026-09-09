"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 购买按钮（§8.2：写操作带幂等键；错误按 code 映射文案） */
export function BuyButton({
  itemId,
  price,
}: {
  itemId: number;
  name: string;
  price: number;
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

  return (
    <div className="flex flex-col gap-1">
      <button
        onClick={buy}
        disabled={state !== "idle"}
        className="min-h-[44px] rounded-full bg-coral px-4 font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {state === "busy"
          ? dict.shop.buying
          : state === "done"
            ? dict.shop.bought
            : dict.shop.buy}
      </button>
      {message && (
        <p role="status" className="text-xs text-sub">
          {message}
        </p>
      )}
    </div>
  );
}
