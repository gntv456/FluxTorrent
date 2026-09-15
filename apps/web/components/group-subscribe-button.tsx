"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 聚合组订阅（0075）：POST /torrents/groups/{gid}/subscribe | /unsubscribe
 *  新版本入组并过审时站内通知（受 notice_prefs.group_new_version 偏好控制） */
export function GroupSubscribeButton({ groupId }: { groupId: number }) {
  const { dict } = useI18n();
  const t = dict.groupsub;
  const [subscribed, setSubscribed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  async function toggle() {
    if (busy) return;
    setBusy(true);
    setMsg(null);
    try {
      if (subscribed) {
        await api.post(`/api/v1/torrents/groups/${groupId}/unsubscribe`, {});
        setSubscribed(false);
        setMsg(t.unsubOk);
      } else {
        await api.post(`/api/v1/torrents/groups/${groupId}/subscribe`, {});
        setSubscribed(true);
        setMsg(t.ok);
      }
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.failed);
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="inline-flex flex-col items-start gap-0.5">
      <button
        type="button"
        onClick={toggle}
        disabled={busy}
        className={`min-h-[30px] rounded-full px-3 text-xs font-bold transition-transform active:scale-[0.97] disabled:opacity-50 ${
          subscribed
            ? "border border-[var(--baozi-orange)] text-[var(--baozi-orange-dark)]"
            : "bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white"
        }`}
      >
        {subscribed ? `🔔 ${t.subscribed}` : `🔕 ${t.subscribe}`}
      </button>
      {msg && (
        <span className="text-[11px] text-sub" role="status">
          {msg}
        </span>
      )}
    </span>
  );
}
