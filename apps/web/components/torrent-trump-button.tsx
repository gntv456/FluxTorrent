"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { Modal } from "@/components/modal";
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
 *  「权限/能力由后端兜底」的既有口径一致）。
 *
 *  表单改覆盖式弹窗（同 0173 的 forum-report）：此前是 `absolute` 下拉小面板，
 *  而「更多操作」本身是带 `overflow-y:auto` 的居中面板，下拉被面板裁掉
 *  并与按钮行叠在一起（站长实测：举报框「被覆盖」）。 */
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
      setNote("");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.reportFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="inline-flex flex-col gap-1">
      <button
        type="button"
        className="sticker"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => {
          setOpen((v) => !v);
          setMsg(null);
        }}
      >
        {t.reportButton}
      </button>
      <Modal open={open} onClose={() => setOpen(false)} title={t.reportTitle}>
        <span className="flex flex-col gap-2">
          <label className="text-xs font-bold text-ink">
            {t.reportReasonLabel}
            <select
              className="mt-1 w-full rounded-[var(--r-sm)] border border-line bg-transparent px-2 py-1 text-xs text-ink"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
            >
              {REASONS.map((r) => (
                <option key={r} value={r}>
                  {labels[r]}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs font-bold text-ink">
            {t.reportNoteLabel}
            <textarea
              className="mt-1 w-full rounded-[var(--r-sm)] border border-line bg-transparent px-2 py-1 text-xs text-ink"
              rows={4}
              maxLength={500}
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
          </label>
          <span className="flex items-center justify-end gap-2">
            <button
              type="button"
              className="min-h-[28px] rounded-full border border-line px-3 text-xs font-bold text-sub transition hover:border-coral hover:text-coral"
              onClick={() => setOpen(false)}
            >
              {t.reportCancel}
            </button>
            <button
              type="button"
              className="min-h-[28px] rounded-full bg-coral px-3 text-xs font-bold text-white transition disabled:opacity-50"
              disabled={busy}
              onClick={() => void submit()}
            >
              {t.reportSubmit}
            </button>
          </span>
        </span>
      </Modal>
      {msg && <span className="text-[11px] text-mint">{msg}</span>}
    </span>
  );
}
