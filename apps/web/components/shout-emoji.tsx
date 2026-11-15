"use client";

import { useEffect, useRef, useState } from "react";
import { useI18n } from "@/i18n/client";

/** 快捷表情（NP shoutbox smile_row 口径）：一排常用 emoji，点击插入输入框光标处。
 *  零迁移零后端——emoji 直接进消息文本，与 BBCode/[emN] 无关。 */
const EMOJIS = [
  "😀", "😂", "🤣", "😊", "😍", "🤔", "😎", "😭",
  "😡", "🥳", "👍", "👎", "👏", "🙏", "💪", "🎉",
  "❤️", "💔", "🔥", "⭐", "🎬", "🎧", "📚", "🌱",
];

export function ShoutEmojiBar({
  onPick,
}: {
  /** 把选中的 emoji 插进输入框（父组件持 ref 回写） */
  onPick: (emoji: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.shoutbox;
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  // 点面板外收起（与 usertools-popover 同款交互）
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <div ref={box} className="relative">
      <button
        type="button"
        aria-label={t.emojiTitle}
        title={t.emojiTitle}
        onClick={() => setOpen((v) => !v)}
        className={
          "flex h-11 w-11 shrink-0 items-center justify-center " +
          "rounded-lg border border-line text-lg " +
          "hover:border-[var(--sky)]"
        }
      >
        🙂
      </button>
      {open && (
        <div
          className={
            "absolute bottom-full left-0 z-30 mb-1 grid w-max " +
            "max-w-[240px] grid-cols-8 gap-0.5 rounded-xl " +
            "border border-line bg-[var(--compat-paper)] p-2 shadow-lg"
          }
        >
          {EMOJIS.map((e) => (
            <button
              key={e}
              type="button"
              onClick={() => {
                onPick(e);
                setOpen(false);
              }}
              className={
                "flex h-7 w-7 items-center justify-center rounded " +
                "text-base hover:bg-[var(--sky-soft)]"
              }
            >
              {e}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
