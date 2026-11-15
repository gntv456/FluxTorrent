"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";

/** 保种认领按钮（旧站 claim 口径：无人认领的保种可一键认领）。
 *  bonus：每保满一天可得（preserve_bonus_per_day）——利益前置，
 *  按钮下直接标「+N/天」，点之前就知道认领换什么。 */
export function ClaimButton({
  torrentId,
  claimed,
  label,
  claimedLabel,
  bonus,
}: {
  torrentId: number;
  claimed: boolean;
  label: string;
  claimedLabel: string;
  bonus?: number;
}) {
  const { dict } = useI18n();
  const [done, setDone] = useState(claimed);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (done) {
    return <Badge tone="success">{claimedLabel}</Badge>;
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
        className="min-h-11 rounded-full bg-sky-deep px-3 text-xs text-[var(--text-on-accent)] disabled:opacity-50"
      >
        {label}
        {bonus ? (
          <i
            className={
              "ml-1 rounded-[6px] bg-white/25 px-1 py-px " +
              "text-[11px] not-italic"
            }
          >
            +{bonus}
          </i>
        ) : null}
      </button>
      {msg && <span className="text-xs text-[var(--danger-text)]">{msg}</span>}
    </span>
  );
}
