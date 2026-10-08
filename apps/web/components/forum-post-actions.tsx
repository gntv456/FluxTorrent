"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Modal } from "@/components/modal";

/** 论坛回帖与帖子级管理操作（从 forum-composer.tsx 按域拆出）：
 *  ReplyBox 回帖、TopicModActions 主题管理（版主）、PostActions 帖子编辑/删除。
 *  发新主题（含悬赏/投票/抽奖）见 forum-composer.tsx。 */

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
  const router = useRouter();
  const { dict } = useI18n();
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
    if (!window.confirm(dict.forums.delTopicConfirm)) return;
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
      <button
        className={btn}
        disabled={busy}
        onClick={() => manage({ sticky: !sticky })}
      >
        {sticky ? dict.forums.unsticky : dict.forums.sticky}
      </button>
      <button
        className={btn}
        disabled={busy}
        onClick={() => manage({ locked: !locked })}
      >
        {locked ? dict.forums.unlock : dict.forums.locked}
      </button>
      <button
        className={btn}
        disabled={busy}
        onClick={() => manage({ digest: !digest })}
      >
        {digest ? dict.forums.undigest : dict.forums.digest}
      </button>
      <select
        value={moveTo}
        onChange={(e) => setMoveTo(e.target.value)}
        className="min-h-[36px] rounded-full border border-line bg-cloud px-3 text-xs outline-none focus:border-sky"
      >
        <option value="">{dict.forums.moveTo}</option>
        {forums.map((f) => (
          <option key={f.id} value={f.id}>
            {f.name}
          </option>
        ))}
      </select>
      {moveTo && (
        <button
          className={btn}
          disabled={busy}
          onClick={() => manage({ move_to_forum_id: Number(moveTo) })}
        >
          {dict.forums.confirmMove}
        </button>
      )}
      <button
        className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-danger disabled:opacity-50"
        disabled={busy}
        onClick={del}
      >
        {dict.forums.delTopic}
      </button>
    </div>
  );
}

/** 帖子操作：作者本人可编辑；版主/postmanage 可编辑+删他人帖（编辑自动 PM 通知） */
export function PostActions({
  postId,
  canMod,
  isSelf,
  initialBody,
}: {
  postId: number;
  canMod: boolean;
  isSelf: boolean;
  initialBody?: string;
}) {
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
    if (!window.confirm(dict.forums.delPostConfirm)) return;
    setBusy(true);
    try {
      await api.del(`/api/v1/forums/posts/${postId}`);
      router.refresh();
    } catch (err) {
      setMsg(err instanceof ApiError ? err.message : dict.common.networkError);
      setBusy(false);
    }
  }

  // 编辑改覆盖式弹窗（与种子编辑 TorrentManage 同口径）：
  // 不再内联展开挤动楼层布局；display:contents 让按钮融入楼层操作行
  return (
    <div className="contents">
      <button className={btn} onClick={() => setEditing(true)}>
        {dict.forums.edit}
      </button>
      {canMod && !isSelf && (
        <button
          className="min-h-[30px] rounded-full border border-line px-3 text-xs font-bold text-danger disabled:opacity-50"
          disabled={busy}
          onClick={del}
        >
          {dict.forums.del}
        </button>
      )}
      {msg && <span className="text-xs text-danger">{msg}</span>}
      <Modal
        open={editing}
        onClose={() => setEditing(false)}
        title={dict.forums.postEditTitle}
      >
        <div className="flex w-full flex-col gap-2">
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            rows={10}
            maxLength={5000}
            autoFocus
            className="min-h-[160px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
          />
          {msg && (
            <p role="alert" className="text-xs text-danger">
              {msg}
            </p>
          )}
          <div className="flex gap-2">
            <button
              className="min-h-[30px] rounded-full bg-sky px-3 text-xs font-bold text-white disabled:opacity-50"
              disabled={busy}
              onClick={save}
            >
              {dict.common.save}
            </button>
            <button className={btn} onClick={() => setEditing(false)}>
              {dict.common.cancel}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
