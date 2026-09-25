"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/**
 * 勋章购买/佩戴/赠送（客户端交互叶子组件 §8.3.2）。
 *
 * ⚠️ 购买口径按 **get_type** 而非「有没有价格」：库里存在 `get_type=2`（站长授予型）
 * 却带 `price=30000` 的勋章（如「保种达人」），旧写法 `price === null ? 不可购买 : 购买`
 * 会渲染出一个点不动的「购买」按钮。1=魔力兑换 / 2=授予 / 3=合成。
 *
 * 0204：佩戴改多佩戴位（上限内 toggle 单枚，上限由站点设置）；赠送放开为
 * 代购式——未拥有也可送（送方付费），任意勋章（含授予/合成型）都显示赠送按钮。
 */
export function MedalActions({
  medalId,
  owned,
  wearing,
  price,
  getType,
  giftFeeBp,
  globalGiftTaxBp,
  onWearChange,
}: {
  medalId: number;
  owned: boolean;
  wearing: boolean;
  price: number | null;
  getType: number;
  /** per-勋章赠送手续费（基点；null = 用全站） */
  giftFeeBp?: number | null;
  /** 全站赠送税率（基点，仅用于费率提示） */
  globalGiftTaxBp?: number;
  /** 佩戴态变化回调（父层刷新「已佩戴 n/N」计数） */
  onWearChange?: () => void;
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
      // 多佩戴：显式 wear 布尔；超限时后端报「佩戴已达上限」
      await api.put("/api/v1/medals/wear", {
        medal_id: medalId,
        wear: on,
      });
      setIsWearing(on);
      setMsg(on ? dict.medals.worn : dict.medals.unworn);
      onWearChange?.();
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
      const r = await api.post<{ tax: number; price: number }>(
        "/api/v1/medals/gift",
        {
          medal_id: medalId,
          to_user: to,
        },
      );
      setMsg(dict.medals2.giftOk.replace("{name}", to));
      const tax = (r as { tax?: number }).tax ?? 0;
      if (tax > 0) {
        setMsg(
          dict.medals2.giftOk.replace("{name}", to) +
            " " +
            dict.medals2.giftFeeNote.replace("{n}", String(tax)),
        );
      }
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    }
  }

  const locked = (
    <span className="btn btn-sm cursor-default opacity-60">
      {getType === 2
        ? dict.medals.grantOnly
        : getType === 3
          ? dict.medals.synthOnly
          : dict.medals.notBuyable}
    </span>
  );

  // 赠送按钮：代购式（0204），未拥有也可送——独立于购买/锁定态
  const giftBtn = (
    <button
      type="button"
      onClick={gift}
      className="btn btn-sm btn-ghost"
      title={
        giftFeeBp != null
          ? dict.medals2.giftFeeRate.replace(
              "{n}",
              String(Math.round(giftFeeBp / 100)),
            )
          : globalGiftTaxBp != null
            ? dict.medals2.giftFeeRate.replace(
                "{n}",
                String(Math.round(globalGiftTaxBp / 100)),
              )
            : undefined
      }
    >
      {dict.medals2.gift}
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
            {giftBtn}
          </>
        ) : buyable ? (
          <>
            <button
              type="button"
              onClick={buy}
              className="btn btn-sm btn-primary"
            >
              {dict.medals.buy}
            </button>
            {giftBtn}
          </>
        ) : (
          <>
            {locked}
            {giftBtn}
          </>
        )}
      </div>
      {msg && <p className="text-xs text-sub">{msg}</p>}
    </div>
  );
}
