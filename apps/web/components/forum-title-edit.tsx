"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Modal } from "@/components/modal";

/** 主题标题编辑（楼主本人或版主可见）。
 *  后端 PUT /api/v1/forums/topics/{id}/title；标题在 topics 表，
 *  PostActions 的编辑只改 posts.body，管不到这一列。 */
export function TopicTitleEdit({
  topicId,
  initialTitle,
}: {
  topicId: number;
  initialTitle: string;
}) {
  const router = useRouter();
  const { dict } = useI18n();
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState(initialTitle);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  async function save() {
    const next = title.trim();
    if (!next) {
      setMsg(dict.forums.titleEditEmpty);
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      await api.put(`/api/v1/forums/topics/${topicId}/title`, {
        title: next,
      });
      setOpen(false);
      router.refresh();
    } catch (err) {
      setMsg(err instanceof ApiError ? err.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const btn =
    "min-h-[30px] rounded-full border border-line px-3 text-xs " +
    "font-bold text-sub active:scale-[0.97]";

  return (
    <>
      <button
        className={btn}
        onClick={() => {
          setTitle(initialTitle);
          setMsg(null);
          setOpen(true);
        }}
      >
        {dict.forums.titleEdit}
      </button>
      <Modal
        open={open}
        onClose={() => setOpen(false)}
        title={dict.forums.titleEdit}
      >
        <div className="flex w-full flex-col gap-2">
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.forums.topicTitle}</span>
            <input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              maxLength={120}
              autoFocus
              className="min-h-[44px] rounded-[var(--r-sm)] border
                border-line bg-[var(--cloud)] px-3 text-sm outline-none
                focus:border-[var(--sky)]"
            />
          </label>
          {msg && (
            <p role="alert" className="text-xs text-[var(--danger-text)]">
              {msg}
            </p>
          )}
          <div className="flex gap-2">
            <button
              className="min-h-[30px] rounded-full bg-[var(--sky)] px-3 text-xs
                font-bold text-[var(--text-on-accent)] disabled:opacity-50"
              disabled={busy}
              onClick={save}
            >
              {dict.common.save}
            </button>
            <button className={btn} onClick={() => setOpen(false)}>
              {dict.common.cancel}
            </button>
          </div>
        </div>
      </Modal>
    </>
  );
}
