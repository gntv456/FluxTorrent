"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 认领复活任务（0073，U3D Graveyard 口径）：POST /resurrections/claim {torrent_id}
 *  后端约束：不能领自己发的种；一种同时只能有一个进行中的复活任务。 */
export function ResurrectButton({ torrentId, name }: { torrentId: number; name: string }) {
  const { dict, currency } = useI18n();
  const t = dict.resurrect;
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  async function claim() {
    if (busy || done) return;
    if (!window.confirm(t.deadNote)) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ torrent_id: number; required_hours: number }>(
        "/api/v1/resurrections/claim",
        { torrent_id: torrentId },
      );
      setDone(t.claimed.replace("{hours}", String(r.required_hours)).replace("{reward}", t.reward));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.claimFailed);
    } finally {
      setBusy(false);
    }
  }

  if (done) {
    return <span className="text-xs font-bold text-mint" title={name}>{done}</span>;
  }
  return (
    <span className="inline-flex flex-col items-start">
      <button
        type="button"
        onClick={claim}
        disabled={busy}
        title={t.note.replace("火花", currency)}
        className="min-h-[36px] rounded-full border border-[var(--baozi-orange-dark)] px-4 text-xs font-bold text-[var(--baozi-orange-dark)] transition-transform active:scale-[0.97] disabled:opacity-50"
      >
        {`🌱 ${t.claimBtn}`}
      </button>
      {msg && (
        <span className="mt-1 text-xs text-danger" role="status">
          {msg}
        </span>
      )}
    </span>
  );
}
