"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 购买按钮（§8.2：写操作带幂等键；错误按 code 映射文案） */
export function BuyButton({
  itemId,
  price,
}: {
  itemId: number;
  name: string;
  price: number;
}) {
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
      setMessage("购买成功～");
    } catch (err) {
      setState("idle");
      if (err instanceof ApiError) {
        setMessage(err.code === 4001 ? "火花不够啦，去做种赚点吧" : err.message);
      } else {
        setMessage("网络异常，请稍后再试");
      }
    }
  }

  return (
    <div className="flex flex-col gap-1">
      <button
        onClick={buy}
        disabled={state !== "idle"}
        className="min-h-[44px] rounded-full bg-coral px-4 font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {state === "busy" ? "购买中…" : state === "done" ? "已购买 ✓" : "购买"}
      </button>
      {message && (
        <p role="status" className="text-xs text-sub">
          {message}
        </p>
      )}
    </div>
  );
}
