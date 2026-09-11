"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 论坛发主题（需 forum_id）——对接 POST /forums/topics */
export function TopicComposer({ forumId }: { forumId: number }) {
  const { dict } = useI18n();
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ topic_id: number }>("/api/v1/forums/topics", {
        forum_id: forumId,
        title: title.trim(),
        body: body.trim(),
      });
      router.push(`/forums/topic/${r.topic_id}`);
    } catch (err) {
      setMsg(
        err instanceof ApiError ? (dict.errors[err.code] ?? err.message) : dict.common.networkError,
      );
      setBusy(false);
    }
  }

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="min-h-[44px] self-start rounded-full bg-coral px-5 text-sm font-bold text-white active:scale-[0.97]"
      >
        {dict.forums.newTopic}
      </button>
    );
  }

  return (
    <form
      onSubmit={submit}
      className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
    >
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.forums.topicTitle}</span>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          required
          maxLength={120}
          className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.forums.topicBody}</span>
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          required
          rows={5}
          maxLength={5000}
          className="min-h-[100px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
        />
      </label>
      {msg && (
        <p role="alert" className="text-sm text-danger">
          {msg}
        </p>
      )}
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sky px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          {busy ? dict.forums.posting : dict.forums.submitTopic}
        </button>
        <button
          type="button"
          onClick={() => setOpen(false)}
          className="min-h-[44px] rounded-full border border-line px-5 text-sm text-sub active:scale-[0.97]"
        >
          {dict.forums.cancel}
        </button>
      </div>
    </form>
  );
}

/** 主题页回复框——对接 POST /forums/topics/{id}/reply */
export function ReplyBox({ topicId }: { topicId: number }) {
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
      });
      setBody("");
      router.refresh();
    } catch (err) {
      setMsg(
        err instanceof ApiError ? (dict.errors[err.code] ?? err.message) : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky";

  return (
    <form
      onSubmit={submit}
      className="flex flex-col gap-2 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
    >
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.forums.replyTitle}</span>
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          required
          rows={3}
          maxLength={5000}
          placeholder={dict.forums.replyPlaceholder}
          className="min-h-[80px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
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
        className="min-h-[44px] self-start rounded-full bg-sky px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {busy ? dict.forums.posting : dict.forums.submitReply}
      </button>
    </form>
  );
}
