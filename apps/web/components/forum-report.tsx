"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Modal } from "@/components/modal";

/**
 * 主题举报（Phase1 / G9 基础）：复用既有 `POST /reports`（ref_type=forum → topics，
 * 后端已做目标存在性校验与 1-500 字理由校验），只补「上下文内一键举报」入口。
 * 提交后收起表单并提示，不再单独维护一套举报表。
 */
export function ReportTopicButton({ topicId }: { topicId: number }) {
  const { dict } = useI18n();
  const [open, setOpen] = useState(false);
  const [reason, setReason] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit() {
    const r = reason.trim();
    if (!r) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/reports", {
        ref_type: "forum",
        ref_id: topicId,
        reason: r,
      });
      setReason("");
      setOpen(false);
      setMsg(dict.usertools.reportOk);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="inline-flex flex-col gap-1">
      <button
        type="button"
        onClick={() => {
          setOpen((v) => !v);
          setMsg(null);
        }}
        aria-expanded={open}
        title={dict.forums.report}
        className="inline-flex min-h-[32px] items-center gap-1 rounded-full border border-line px-3 text-xs font-bold text-sub transition hover:border-coral hover:text-coral"
      >
        <span aria-hidden="true">🚩</span>
        {dict.forums.report}
      </button>
      {/* 0173：举报改覆盖式弹窗，不再以小面板嵌在操作行里 */}
      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={dict.forums.report}
      >
        <span className="flex flex-col gap-2">
          <textarea
            rows={4}
            value={reason}
            maxLength={500}
            onChange={(e) => setReason(e.target.value)}
            placeholder={dict.usertools.reportReason}
            aria-label={dict.usertools.reportReason}
            className="w-full rounded-[var(--r-sm)] border border-line bg-transparent px-2 py-1 text-xs text-ink outline-none focus:border-sky"
          />
          <span className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void submit()}
              disabled={busy || !reason.trim()}
              className="min-h-[28px] rounded-full bg-coral px-3 text-xs font-bold text-white transition disabled:opacity-50"
            >
              {dict.usertools.reportSubmit}
            </button>
            <span className="num text-[11px] text-sub">
              {reason.length}/500
            </span>
          </span>
        </span>
      </Modal>
      {msg && <span className="text-[11px] text-mint">{msg}</span>}
    </span>
  );
}
