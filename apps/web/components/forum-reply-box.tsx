"use client";

import { useState, useRef } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { TorrentInsertRow } from "@/components/forum-torrent-insert";
import { appendVideoBlock } from "@/lib/forum-video";
import { Modal } from "@/components/modal";
import { EditorToolbar } from "@/components/editor-toolbar";

/** 长类名提常量（行宽门禁 ≤80） */
const TXT_CLS =
  "min-h-[180px] rounded-[var(--r-sm)] border border-line bg-cloud " +
  "px-3 py-2 text-sm outline-none focus:border-sky";
const SUBMIT_CLS =
  "min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold " +
  "text-white active:scale-[0.97] disabled:opacity-50";
const REPLY_BADGE =
  "flex items-center gap-2 rounded-[var(--r-sm)] bg-sky-soft " +
  "px-2.5 py-1 text-xs text-ink";
const COUNTER_CLS = "num text-xs text-sub";

/** 主题页回复框（2026-11-12 弹窗化，参考机窝编辑器形态）：
 *  页面底部原位常驻改成「回复」按钮弹窗——阅读动线不被输入框打断。
 *  弹窗内：定向回复徽标 → 正文 textarea（字数实时统计）→ 引用种子
 *  插入行 → 提交。对接 POST /forums/topics/{id}/reply；楼中楼（0163）
 *  带 replyTarget 时为定向回复，提交携带 reply_to，成功后清 ?reply_to=。 */
export function ReplyBox({
  topicId,
  replyTarget,
}: {
  topicId: number;
  replyTarget?: { id: number; username: string | null } | null;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [open, setOpen] = useState(false);
  // 定向回复进来（?reply_to=N）时弹窗自动展开——动线不要求用户再点一次
  const [autoOpened, setAutoOpened] = useState(false);
  if (replyTarget && !autoOpened) {
    setAutoOpened(true);
    setOpen(true);
  }
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const areaRef = useRef<HTMLTextAreaElement>(null);

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
      setOpen(false);
      // 楼中楼回复完成后清掉 ?reply_to=，避免刷新后仍处于定向回复态
      if (replyTarget) router.replace(`/forums/topic/${topicId}`);
      router.refresh();
    } catch (err) {
      setMsg(
        err instanceof ApiError
          ? (apiErrorMessage(dict, err))
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex justify-center">
      <button
        type="button"
        onClick={() => setOpen(true)}
        className={
          "min-h-[44px] w-full max-w-[320px] rounded-full bg-sky " +
          "px-6 text-sm font-bold text-white active:scale-[0.98]"
        }
      >
        {dict.forums.replyTitle}
      </button>
      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={dict.forums.replyTitle}
        size="lg"
      >
        <form onSubmit={submit} className="flex flex-col gap-3">
          {replyTarget && (
            <div className={REPLY_BADGE}>
              <span>
                {dict.forums.replyBadge.replace(
                  "{n}",
                  String(replyTarget.id),
                )}
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
            <span className="sr-only">{dict.forums.replyTitle}</span>
            <textarea
              ref={areaRef}
              value={body}
              onChange={(e) => setBody(e.target.value)}
              required
              rows={8}
              autoFocus
              maxLength={5000}
              placeholder={dict.forums.replyPlaceholder}
              className={TXT_CLS}
            />
          </label>
          {/* 工具栏（12 键，选区包裹/光标插入）+ 引用种子插入行 */}
          <EditorToolbar areaRef={areaRef} onChange={(t) => setBody(t)}>
            <TorrentInsertRow
              onInsert={(text) => setBody((b) => appendVideoBlock(b, text))}
            />
          </EditorToolbar>
          {msg && (
            <p role="alert" className="text-sm text-[var(--danger-text)]">
              {msg}
            </p>
          )}
          <div className="flex items-center justify-between gap-3">
            <span className={COUNTER_CLS}>
              {body.length} / 5000
            </span>
            <button
              type="submit"
              disabled={busy}
              className={SUBMIT_CLS}
            >
              {busy ? dict.forums.posting : dict.forums.submitReply}
            </button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
