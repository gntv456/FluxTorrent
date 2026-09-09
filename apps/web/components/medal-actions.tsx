"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

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
  const { dict } = useI18n();
  const [isOwned, setIsOwned] = useState(owned);
  const [isWearing, setIsWearing] = useState(wearing);
  const [msg, setMsg] = useState<string | null>(null);
  const [idem] = useState(() => `web-medal-${medalId}-${crypto.randomUUID()}`);

  async function buy() {
    setMsg(null);
    try {
      await api.post("/api/v1/medals/buy", {
        medal_id: medalId,
        idempotency_key: idem,
      });
      setIsOwned(true);
      setMsg(dict.medals.buyOk);
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    }
  }

  async function wear(on: boolean) {
    setMsg(null);
    try {
      await api.put("/api/v1/medals/wear", { medal_id: on ? medalId : null });
      setIsWearing(on);
      setMsg(on ? dict.medals.worn : dict.medals.unworn);
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
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
          {price === null ? dict.medals.notBuyable : dict.medals.buy}
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
        {isWearing ? dict.medals.wearing : dict.medals.wear}
      </button>
      {msg && <p className="text-xs text-sub">{msg}</p>}
    </div>
  );
}
