"use client";

import { useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { clientUuid } from "@/lib/client-uuid";
import { toast } from "@/components/ui/toast";

/** 邀请码的「生成 / 魔力兑换」两个写动作（从 invite-manager.tsx 按域拆出）。
 *
 *  两者共用一枚 busy 闸与幂等键：兑换走 clientUuid 当幂等键（与 /shop/buy
 *  同口径，防网络重试双扣魔力）；生成没有幂等语义，每次点击都是新码。 */
export function useInviteIssue({
  onDone,
  onError,
  replayedText,
}: {
  /** 生成/兑换成功后刷新列表 */
  onDone: () => Promise<void> | void;
  /** 失败出口（调用方给字典文案） */
  onError: (msg: string) => void;
  /** 兑换重放时的提示文案 */
  replayedText: string;
}) {
  const [busy, setBusy] = useState(false);
  const idemRef = useRef<string | null>(null);

  async function issue() {
    setBusy(true);
    try {
      const r = await api.post<{ code: string }>("/api/v1/invites", {});
      toast.success(r.code);
      await onDone();
    } catch (e) {
      onError(e instanceof ApiError ? e.message : "");
    } finally {
      setBusy(false);
    }
  }

  async function redeem() {
    setBusy(true);
    try {
      // 幂等键：每次点击生成新 UUID（与 /shop/buy 同口径，防网络重试双扣款）
      const r = await api.post<{ code?: string; replayed?: boolean }>(
        "/api/v1/invites/redeem",
        { idempotency_key: (idemRef.current ??= clientUuid()) },
      );
      toast.success(r.code ?? replayedText);
      await onDone();
    } catch (e) {
      onError(e instanceof ApiError ? e.message : "");
    } finally {
      setBusy(false);
    }
  }

  return { busy, issue, redeem };
}
