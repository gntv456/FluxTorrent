"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import {
  BTN_LINE,
  BTN_SKY,
  INPUT_CLS,
} from "./forum-structure-types";

/** 左栏底部「＋ 新建分区」（样图按钮位：全宽幽灵按钮）。
 *
 *  点击原地展开输入行：Enter 提交 / Esc 收起。提交走父页的 run()
 *  包装（失败有提示，成功后整页刷新，新分区出现在列表里）。 */
export function ForumCatCreate({
  busy,
  onCreate,
}: {
  busy: boolean;
  onCreate: (name: string) => void;
}) {
  const { dict } = useI18n();
  const f = dict.adminForums;
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");

  function submit() {
    const v = name.trim();
    if (!v) return;
    onCreate(v);
    setName("");
    setOpen(false);
  }

  if (!open) {
    return (
      <button
        type="button"
        className={`${BTN_LINE} mt-2 min-h-[32px] w-full text-xs`}
        onClick={() => setOpen(true)}
      >
        {f.catNewBtn}
      </button>
    );
  }

  return (
    <div className="mt-2 flex items-center gap-1">
      <input
        autoFocus
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") submit();
          if (e.key === "Escape") setOpen(false);
        }}
        placeholder={f.newNamePh}
        className={`${INPUT_CLS} min-h-[32px] min-w-0 text-xs`}
      />
      <button
        type="button"
        disabled={busy || !name.trim()}
        className={`${BTN_SKY} min-h-[32px] shrink-0 px-3 text-xs`}
        onClick={submit}
      >
        {f.createBtn}
      </button>
      <button
        type="button"
        className={`${BTN_LINE} min-h-[32px] shrink-0 px-2 text-xs`}
        onClick={() => {
          setOpen(false);
          setName("");
        }}
      >
        {f.cancel}
      </button>
    </div>
  );
}
