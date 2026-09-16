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

/** 主题管理操作（版主/postmanage）——置顶/锁定/移动/删主题 */
export function TopicModActions({
  topicId,
  sticky,
  locked,
  digest,
  forums,
}: {
  topicId: number;
  sticky: boolean;
  locked: boolean;
  digest: boolean;
  forums: { id: number; name: string }[];
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [moveTo, setMoveTo] = useState("");

  async function manage(payload: Record<string, unknown>) {
    setBusy(true);
    try {
      await api.post(`/api/v1/forums/topics/${topicId}/manage`, payload);
      router.refresh();
    } catch {
      // 权限或网络错误静默（按钮本就只对 can_mod 渲染）
    } finally {
      setBusy(false);
    }
  }

  async function del() {
    if (!window.confirm("删除主题将收回发帖奖励，且不可恢复，确认？")) return;
    setBusy(true);
    try {
      await api.del(`/api/v1/forums/topics/${topicId}`);
      router.push("/forums");
    } catch {
      setBusy(false);
    }
  }

  const btn =
    "min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-sub active:scale-[0.97] disabled:opacity-50";

  return (
    <div className="flex flex-wrap items-center gap-2">
      <button className={btn} disabled={busy} onClick={() => manage({ sticky: !sticky })}>
        {sticky ? "取消置顶" : "置顶"}
      </button>
      <button className={btn} disabled={busy} onClick={() => manage({ locked: !locked })}>
        {locked ? "解锁" : "锁定"}
      </button>
      <button className={btn} disabled={busy} onClick={() => manage({ digest: !digest })}>
        {digest ? "取消精华" : "加精"}
      </button>
      <select
        value={moveTo}
        onChange={(e) => setMoveTo(e.target.value)}
        className="min-h-[36px] rounded-full border border-line bg-cloud px-3 text-xs outline-none focus:border-sky"
      >
        <option value="">移动到…</option>
        {forums.map((f) => (
          <option key={f.id} value={f.id}>
            {f.name}
          </option>
        ))}
      </select>
      {moveTo && (
        <button className={btn} disabled={busy} onClick={() => manage({ move_to_forum_id: Number(moveTo) })}>
          确认移动
        </button>
      )}
      <button
        className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-danger disabled:opacity-50"
        disabled={busy}
        onClick={del}
      >
        删除主题
      </button>
    </div>
  );
}

/** 帖子操作：作者本人可编辑；版主/postmanage 可编辑+删他人帖（编辑自动 PM 通知） */
export function PostActions({ postId, canMod, isSelf, initialBody }: { postId: number; canMod: boolean; isSelf: boolean; initialBody?: string }) {
  const { dict } = useI18n();
  const router = useRouter();
  const [editing, setEditing] = useState(false);
  const [body, setBody] = useState(initialBody ?? "");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  if (!canMod && !isSelf) return null;

  const btn =
    "min-h-[30px] rounded-full border border-line px-3 text-xs font-bold text-sub active:scale-[0.97] disabled:opacity-50";

  async function save() {
    setBusy(true);
    setMsg(null);
    try {
      await api.put(`/api/v1/forums/posts/${postId}`, { body: body.trim() });
      setEditing(false);
      router.refresh();
    } catch (err) {
      setMsg(err instanceof ApiError ? err.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function del() {
    if (!window.confirm("删除该帖（仅版主可操作），确认？")) return;
    setBusy(true);
    try {
      await api.del(`/api/v1/forums/posts/${postId}`);
      router.refresh();
    } catch (err) {
      setMsg(err instanceof ApiError ? err.message : dict.common.networkError);
      setBusy(false);
    }
  }

  if (editing) {
    return (
      <div className="mt-2 flex flex-col gap-2">
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          rows={4}
          maxLength={5000}
          className="min-h-[80px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
        />
        {msg && <p role="alert" className="text-xs text-danger">{msg}</p>}
        <div className="flex gap-2">
          <button className="min-h-[30px] rounded-full bg-sky px-3 text-xs font-bold text-white disabled:opacity-50" disabled={busy} onClick={save}>
            保存
          </button>
          <button className={btn} onClick={() => setEditing(false)}>
            取消
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="mt-2 flex items-center gap-2">
      <button className={btn} onClick={() => setEditing(true)}>
        编辑
      </button>
      {canMod && !isSelf && (
        <button className="min-h-[30px] rounded-full border border-line px-3 text-xs font-bold text-danger disabled:opacity-50" disabled={busy} onClick={del}>
          删除
        </button>
      )}
      {msg && <span className="text-xs text-danger">{msg}</span>}
    </div>
  );
}
