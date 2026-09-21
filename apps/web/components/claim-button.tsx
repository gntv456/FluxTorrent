"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 保种认领按钮（旧站 claim 口径：无人认领的保种可一键认领） */
export function ClaimButton({
  torrentId,
  claimed,
  label,
  claimedLabel,
}: {
  torrentId: number;
  claimed: boolean;
  label: string;
  claimedLabel: string;
}) {
  const { dict } = useI18n();
  const [done, setDone] = useState(claimed);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (done) {
    return <span className="sticker bg-mint/30 text-ink">{claimedLabel}</span>;
  }
  return (
    <span className="flex flex-col items-end gap-1">
      <button
        type="button"
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          setMsg(null);
          try {
            await api.post("/api/v1/preserve/claim", { torrent_id: torrentId });
            setDone(true);
          } catch (e) {
            setMsg(
              e instanceof ApiError ? e.message : dict.common.networkError,
            );
          } finally {
            setBusy(false);
          }
        }}
        className="min-h-[32px] rounded-full bg-sky-deep px-3 text-xs text-white disabled:opacity-50"
      >
        {label}
      </button>
      {msg && <span className="text-xs text-danger">{msg}</span>}
    </span>
  );
}
