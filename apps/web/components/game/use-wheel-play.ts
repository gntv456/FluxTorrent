"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, isRetryableNetError } from "@/lib/api-client";
import { clientUuid } from "@/lib/client-uuid";
import { fmtCur } from "@/i18n/config";
import { wheelPrizeName } from "@/lib/wheel-prize";
import type { WheelDrawResult } from "@/lib/wheel-batch";

export type FlashKind = "win" | "lose" | "tie" | "jackpot";

export interface WheelFlash {
  kind: FlashKind;
  text: string;
}

/**
 * 单抽状态机：请求 → 等盘面停格 → 结算文案 →（一直抽时）续下一局。
 * 幂等键纪律与九宫格一致：同一次抽奖意图复用同一 key，
 * 只有确认结算后才生成新键；网络/超时保留 key 供重查。
 */
export function useWheelPlay({
  ticket,
  labels,
  currency,
  describeError,
  onSettled,
}: {
  ticket: number;
  labels: {
    jackpot: string;
    win: string;
    itemWin: string;
    fallback: string;
    again: string;
    thanks: string;
  };
  currency: string;
  describeError: (e: unknown) => string;
  onSettled: () => void;
}) {
  const [res, setRes] = useState<WheelDrawResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [flash, setFlash] = useState<WheelFlash | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [unknown, setUnknown] = useState(false);
  const [auto, setAuto] = useState(false);
  const [sessionPlays, setSessionPlays] = useState(0);
  /** 每次拿到结果都 +1：连开两把同扇区时，盘面仍要重新转一圈 */
  const [spinSeq, setSpinSeq] = useState(0);

  const idem = useRef("");
  const busyRef = useRef(false);
  const autoRef = useRef(false);
  const timer = useRef<number | null>(null);
  const resultRef = useRef<WheelDrawResult | null>(null);

  useEffect(
    () => () => {
      if (timer.current) window.clearTimeout(timer.current);
    },
    [],
  );

  const stopAuto = useCallback(() => {
    autoRef.current = false;
    setAuto(false);
    if (timer.current) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
  }, []);

  const draw = useCallback(
    async (reuse = false) => {
      if (busyRef.current) return;
      busyRef.current = true;
      setBusy(true);
      setErr(null);
      setFlash(null);
      setRes(null);
      setUnknown(false);
      resultRef.current = null;
      if (!reuse || !idem.current) idem.current = clientUuid();
      try {
        const r = await api.post<WheelDrawResult>(
          "/api/v1/games/wheel",
          { idempotency_key: idem.current },
        );
        resultRef.current = r;
        setRes(r);
        setSpinSeq((n) => n + 1);
      } catch (e) {
        setErr(describeError(e));
        // 只有网络中断/5xx 才值得复用同一幂等键重试；余额不足、限次、
        // 「该局已受理」这类 4xx 重试不会得到不同结果，不能给重试按钮。
        setUnknown(isRetryableNetError(e));
        busyRef.current = false;
        setBusy(false);
        stopAuto();
      }
    },
    [describeError, stopAuto],
  );

  /** 盘面停格后结算：出闪字、刷新总览，并在一直抽模式续下一局。 */
  const landed = useCallback(() => {
    busyRef.current = false;
    setBusy(false);
    setSessionPlays((n) => n + 1);
    const r = resultRef.current;
    if (!r) return;
    const won = r.value ?? r.payout;
    const itemWin = r.kind === "item" && won > 0;
    const fallback = Boolean(r.fell_back) || r.kind === "fallback";
    // 「头奖」只给魔力档 ≥10× 票价：物品档走 win 分支 —— 此前物品档
    // 等值过线会一边放头奖特效、一边播「获得物品」文案，视觉与文案打架。
    const kind: FlashKind = itemWin
      ? "win"
      : won >= ticket * 10
        ? "jackpot"
        : won > ticket
          ? "win"
          : won === ticket
            ? "tie"
            : "lose";
    setFlash({
      kind,
      text:
        fallback
          ? labels.fallback
          : itemWin
          ? fmtCur(
              labels.itemWin,
              { prize: r.prize, value: won },
              currency,
            )
          : won >= ticket * 10
            ? fmtCur(
                labels.jackpot,
                { prize: wheelPrizeName(r), net: won - ticket },
                currency,
              )
            : won > ticket
              ? fmtCur(
                  labels.win,
                  { prize: wheelPrizeName(r), net: won - ticket },
                  currency,
                )
              : won === ticket
                ? labels.again
                : labels.thanks,
    });
    onSettled();
    if (autoRef.current) {
      timer.current = window.setTimeout(() => {
        if (autoRef.current) void draw();
      }, 650);
    }
  }, [ticket, labels, currency, onSettled, draw]);

  const toggleAuto = useCallback(() => {
    if (autoRef.current) {
      stopAuto();
      return;
    }
    autoRef.current = true;
    setAuto(true);
    void draw();
  }, [draw, stopAuto]);

  return {
    res,
    busy,
    flash,
    err,
    unknown,
    auto,
    sessionPlays,
    spinSeq,
    draw,
    landed,
    toggleAuto,
    stopAuto,
  };
}
