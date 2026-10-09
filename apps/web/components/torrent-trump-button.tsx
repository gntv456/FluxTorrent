"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

const REASONS = [
  "bad_quality",
  "dead",
  "wrong_content",
  "duplicate",
  "other",
];

/** 举报种子（0336 trumping，PTP/GGn 口径）：POST /torrents/{id}/trump。
 *  站型未启用 trumping 时后端返回 400 —— 前端不预判能力开关（与仓库
 *  「权限/能力由后端兜底」的既有口径一致）。 */
export function TrumpButton({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const t = dict.tdetail;
  const [open, setOpen] = useState(false);
  const [reason, setReason] = useState("other");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const labels: Record<string, string> = {
    bad_quality: t.reasonBadQuality,
    dead: t.reasonDead,
    wrong_content: t.reasonWrongContent,
    duplicate: t.reasonDuplicate,
    other: t.reasonOther,
  };

  async function submit() {
    setBusy(true);
    try {
      await api.post(`/api/v1/torrents/${torrentId}/trump`, {
        reason,
        note,
      });
      setMsg(t.reportDone);
      setOpen(false);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.reportFail);
    } finally {
      setBusy(false);
    }
  }

  if (msg) {
    return <span className="text-xs text-sub">{msg}</span>;
  }

  return (
    <span className="relative inline-block">
      <button
        type="button"
        className="sticker"
        onClick={() => setOpen((v) => !v)}
      >
        {t.reportButton}
      </button>
      {open && (
        <div
          className="absolute right-0 z-20 mt-1 w-56 rounded
            border bg-white p-2 text-xs shadow"
        >
          <p className="mb-1 font-bold">{t.reportTitle}</p>
          <select
            className="mb-1 w-full"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          >
            {REASONS.map((r) => (
              <option key={r} value={r}>
                {labels[r]}
              </option>
            ))}
          </select>
          <textarea
            className="mb-1 w-full"
            rows={2}
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <div className="flex justify-end gap-1">
            <button
              type="button"
              className="sticker"
              onClick={() => setOpen(false)}
            >
              {t.reportCancel}
            </button>
            <button
              type="button"
              className="sticker"
              disabled={busy}
              onClick={submit}
            >
              {t.reportSubmit}
            </button>
          </div>
        </div>
      )}
    </span>
  );
}
