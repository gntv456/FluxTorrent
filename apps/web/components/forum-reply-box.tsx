"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 长类名提常量（行宽门禁 ≤80） */
/** 长类名提常量（行宽门禁 ≤80；原 forum-post-actions 基线债随迁） */
const FORM_CLS =
  "flex flex-col gap-2 rounded-[var(--r-lg)] border border-line " +
  "bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]";
const TXT_CLS =
  "min-h-[80px] rounded-[var(--r-sm)] border border-line bg-cloud " +
  "px-3 py-2 text-sm outline-none focus:border-sky";
const SUBMIT_CLS =
  "min-h-[44px] self-start rounded-full bg-sky px-5 text-sm font-bold " +
  "text-white active:scale-[0.97] disabled:opacity-50";

const REPLY_BADGE =
  "flex items-center gap-2 rounded-[var(--r-sm)] bg-sky-soft " +
  "px-2.5 py-1 text-xs text-ink";

/** 主题页回复框——对接 POST /forums/topics/{id}/reply；
 *  楼中楼（0163）：带 replyTarget 时为定向回复，提交携带 reply_to */
export function ReplyBox({
  topicId,
  replyTarget,
}: {
  topicId: number;
  replyTarget?: { id: number; username: string | null } | null;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      await api.post(`/api/v1/forums/topics/${topicId}/reply`, {
        body: body.trim(),
        reply_to: replyTarget?.id ?? null,
      });
      setBody("");
      // 楼中楼回复完成后清掉 ?reply_to=，避免刷新后仍处于定向回复态
      if (replyTarget) router.replace(`/forums/topic/${topicId}`);
      router.refresh();
    } catch (err) {
      setMsg(
        err instanceof ApiError
          ? (dict.errors[err.code] ?? err.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={submit}
      className={FORM_CLS}
    >
      {replyTarget && (
        <div className={REPLY_BADGE}>
          <span>
            {dict.forums.replyBadge.replace("{n}", String(replyTarget.id))}
            {replyTarget.username ? ` @${replyTarget.username}` : ""}
          </span>
          <Link
            href={`/forums/topic/${topicId}`}
            className="font-bold text-sky hover:underline"
          >
            {dict.common.cancel}
          </Link>
        </div>
      )}
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.forums.replyTitle}</span>
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          required
          rows={3}
          maxLength={5000}
          placeholder={dict.forums.replyPlaceholder}
          className={TXT_CLS}
        />
      </label>
      {msg && (
        <p role="alert" className="text-sm text-danger">
          {msg}
        </p>
      )}
      <button
        type="submit"
        disabled={busy}
        className={SUBMIT_CLS}
      >
        {busy ? dict.forums.posting : dict.forums.submitReply}
      </button>
    </form>
  );
}

