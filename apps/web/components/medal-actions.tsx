"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/**
 * 勋章购买/佩戴（客户端交互叶子组件 §8.3.2）。
 *
 * ⚠️ 购买口径按 **get_type** 而非「有没有价格」：库里存在 `get_type=2`（站长授予型）
 * 却带 `price=30000` 的勋章（如「保种达人」），旧写法 `price === null ? 不可购买 : 购买`
 * 会渲染出一个点不动的「购买」按钮。1=魔力兑换 / 2=授予 / 3=合成。
 */
export function MedalActions({
  medalId,
  owned,
  wearing,
  price,
  getType,
}: {
  medalId: number;
  owned: boolean;
  wearing: boolean;
  price: number | null;
  getType: number;
}) {
  const { dict } = useI18n();
  const [isOwned, setIsOwned] = useState(owned);
  const [isWearing, setIsWearing] = useState(wearing);
  const [msg, setMsg] = useState<string | null>(null);
  const [idem] = useState(() => `web-medal-${medalId}-${crypto.randomUUID()}`);
  const buyable = getType === 1 && price !== null;

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

  async function gift() {
    const to = prompt(dict.medals2.giftTo)?.trim();
    if (!to) {
      if (to !== null) setMsg(dict.medals2.giftInvalid);
      return;
    }
    setMsg(null);
    try {
      await api.post("/api/v1/medals/gift", {
        medal_id: medalId,
        to_user: to,
      });
      setMsg(dict.medals2.giftOk.replace("{name}", to));
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    }
  }

  const locked = isOwned ? null : (
    <button type="button" className="btn btn-sm" disabled>
      {getType === 2
        ? dict.medals.grantOnly
        : getType === 3
          ? dict.medals.synthOnly
          : dict.medals.notBuyable}
    </button>
  );

  return (
    <div className="ml-auto flex flex-col items-end gap-1">
      <div className="flex gap-1.5">
        {isOwned ? (
          <>
            <button
              type="button"
              onClick={() => wear(!isWearing)}
              className={`btn btn-sm${isWearing ? " btn-own" : ""}`}
            >
              {isWearing ? dict.medals.wearing : dict.medals.wear}
            </button>
            <button
              type="button"
              onClick={gift}
              className="btn btn-sm btn-ghost"
            >
              {dict.medals2.gift}
            </button>
          </>
        ) : buyable ? (
          <button
            type="button"
            onClick={buy}
            className="btn btn-sm btn-primary"
          >
            {dict.medals.buy}
          </button>
        ) : (
          locked
        )}
      </div>
      {msg && <p className="text-xs text-sub">{msg}</p>}
    </div>
  );
}
