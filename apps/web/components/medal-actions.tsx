"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 勋章购买/佩戴（客户端交互叶子组件 §8.3.2） */
export function MedalActions({
  medalId,
  owned,
  wearing,
  price,
}: {
  medalId: number;
  owned: boolean;
  wearing: boolean;
  price: number | null;
}) {
  const [isOwned, setIsOwned] = useState(owned);
  const [isWearing, setIsWearing] = useState(wearing);
  const [msg, setMsg] = useState<string | null>(null);

  async function buy() {
    setMsg(null);
    try {
      await api.post("/api/v1/medals/buy", {
        medal_id: medalId,
        idempotency_key: `web-medal-${medalId}-${Date.now()}`,
      });
      setIsOwned(true);
      setMsg("收入囊中！");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "网络异常");
    }
  }

  async function wear(on: boolean) {
    setMsg(null);
    try {
      await api.put("/api/v1/medals/wear", { medal_id: on ? medalId : null });
      setIsWearing(on);
      setMsg(on ? "已佩戴" : "已摘下");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "网络异常");
    }
  }

  if (!isOwned) {
    return (
      <div className="flex flex-col gap-1">
        <button
          onClick={buy}
          disabled={price === null}
          className="min-h-[44px] rounded-full bg-coral px-3 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-40"
        >
          {price === null ? "不可购买" : "购买"}
        </button>
        {msg && <p className="text-xs text-sub">{msg}</p>}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-1">
      <button
        onClick={() => wear(!isWearing)}
        className={`min-h-[44px] rounded-full px-3 text-sm font-bold active:scale-[0.97] ${
          isWearing ? "bg-mint text-white" : "border border-line bg-cloud text-ink"
        }`}
      >
        {isWearing ? "佩戴中 ✓" : "佩戴"}
      </button>
      {msg && <p className="text-xs text-sub">{msg}</p>}
    </div>
  );
}
