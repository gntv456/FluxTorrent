"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 订阅内容实体（0332 厂牌 / 0334 字幕组）：POST
 *  /networks/{id}/subscribe | /unsubscribe。
 *  该端点按 id 操作 content_networks 行、不校验 kind，故出品方与字幕组
 *  共用同一按钮。新作过审时发站内信（network_/subgroup_new_release）。
 *  P-588：初始态由 /networks/{id} 的 subscribed 带回（同 P-583 组订阅），
 *  否则刷新后恒显示「订阅」，要退订得先点一次订阅再点退订。 */
export function SubscribeNetworkButton({
  networkId,
  initialSubscribed = false,
  scope = "subtitleGroup",
}: {
  networkId: number;
  initialSubscribed?: boolean;
  scope?: "network" | "subtitleGroup";
}) {
  const { dict } = useI18n();
  const t = scope === "network" ? dict.networks : dict.subtitleGroups;
  // 成功/失败提示两处共用（文案与实体 kind 无关），按钮标题按页面取。
  const msgT = dict.subtitleGroups;
  const [on, setOn] = useState(initialSubscribed);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  async function toggle() {
    if (busy) return;
    setBusy(true);
    setMsg(null);
    try {
      if (on) {
        await api.post(`/api/v1/networks/${networkId}/unsubscribe`, {});
        setOn(false);
        setMsg(msgT.unsubOk);
      } else {
        await api.post(`/api/v1/networks/${networkId}/subscribe`, {});
        setOn(true);
        setMsg(msgT.ok);
      }
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : msgT.failed);
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="inline-flex items-center gap-2">
      <button
        type="button"
        onClick={toggle}
        disabled={busy}
        className={`min-h-[30px] rounded-full px-3 text-xs font-bold
          transition-transform active:scale-[0.97] disabled:opacity-50 ${
            on
              ? "border border-[var(--brand-orange)] " +
                "text-[var(--brand-orange-dark)]"
              : "bg-[linear-gradient(135deg,var(--brand-orange-bright)," +
                "var(--brand-orange))] text-white"
          }`}
      >
        {on ? `🔔 ${t.subscribed}` : `🔕 ${t.subscribe}`}
      </button>
      {msg && (
        <span className="text-[11px] text-sub" role="status">
          {msg}
        </span>
      )}
    </span>
  );
}
