"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import {
  BTN_SKY,
  INPUT_CLS,
  TAG_CLS,
  type ModTuple,
} from "./forum-structure-types";

/** 抽屉里的版主任免区（从版块表搬来）。
 *
 *  样图版块表只有 编辑/排序 两个操作，版主任免收进抽屉：chips 列出
 *  现任版主（× 移除），输入框任命。仅编辑态渲染——新建的版块还没有
 *  id，建完再进抽屉任命。 */
export function ForumModsField({
  forumId,
  mods,
  busy,
  run,
}: {
  forumId: number;
  mods: ModTuple[];
  busy: boolean;
  run: (fn: () => Promise<void>, ok: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminForums;
  const [draft, setDraft] = useState("");
  // P2 清偿：免版主二次确认，window.confirm 退役
  const ask = useConfirm();

  const mine = mods.filter((m) => m[0] === forumId);

  function add() {
    const username = draft.trim();
    if (!username) return;
    run(
      async () => {
        await api.post(`/api/v1/admin/forums/${forumId}/mods`, { username });
        setDraft("");
      },
      fmt(t.modAdded, { name: username }),
    );
  }

  function remove(uid: number, uname: string) {
    ask.open({
      title: t.modRemoveTip,
      message: fmt(t.modRemoved, { name: uname }),
      run: () =>
        run(
          async () => {
            await api.del(`/api/v1/admin/forums/${forumId}/mods/${uid}`);
          },
          fmt(t.modRemoved, { name: uname }),
        ),
    });
  }

  return (
    <div className="mb-3 mt-4 border-t border-line pt-3">
      <span className="mb-1 block text-xs text-sub">{t.fldMods}</span>
      <div className="mb-2 flex flex-wrap gap-1">
        {mine.length === 0 && (
          <span className="text-xs text-sub">{t.modsNone}</span>
        )}
        {mine.map((m) => (
          <span key={m[1]} className={TAG_CLS}>
            {m[2]}
            <button
              type="button"
              disabled={busy}
              className="font-bold text-danger"
              title={t.modRemoveTip}
              onClick={() => remove(m[1], m[2])}
            >
              ×
            </button>
          </span>
        ))}
      </div>
      <div className="flex items-center gap-1">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") add();
          }}
          placeholder={t.usernamePh}
          className={`${INPUT_CLS} min-h-[32px] min-w-0 text-xs`}
        />
        <button
          type="button"
          disabled={busy || !draft.trim()}
          className={`${BTN_SKY} min-h-[32px] shrink-0 px-3 text-xs`}
          onClick={add}
        >
          {t.modAddBtn}
        </button>
      </div>
      <AskDialog {...ask} />
    </div>
  );
}
