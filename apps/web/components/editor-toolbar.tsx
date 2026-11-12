"use client";

import { useRef, type ReactNode, type RefObject } from "react";
import { useI18n } from "@/i18n/client";

/** 回复/发帖编辑器工具栏（2026-11-12，参考机窝编辑器形态）。
 *
 *  每键 = 一个 Markdown 插入动作，作用于 textarea 当前选区：
 *  有选中 → 包裹（wrap）；无选中 → 光标处插入占位（insert）。
 *  只收 **渲染器真支持的语法**（forum-markdown INLINE_RE + 块级），
 *  不摆渲染不出来的死按钮。textarea 由父组件 ref 传入（受控 value，
 *  插入后回写 onChange 并恢复光标）。
 *
 *  键位清单（12）：H2 标题 / B 加粗 / I 斜体 / S 删除线 / ` 行内码 /
 *  代码块 / 引用 / 链接 / @提及 / 列表 / 视频 / 表情（emoji 面板）。 */

export interface ToolbarHandle {
  wrap: (before: string, after: string, placeholder: string) => void;
  insert: (text: string, selectPlaceholder?: string) => void;
}

const BTN =
  "inline-flex h-7 min-w-7 items-center justify-center rounded px-1.5 " +
  "text-xs font-bold text-sub hover:bg-[var(--surface-sunken)] " +
  "hover:text-[var(--sky)]";

const EMOJIS = [
  "😀", "😂", "🤣", "😊", "😍", "🤔", "😎", "😢", "😭", "😡",
  "👍", "👎", "👏", "🙏", "💪", "🔥", "⭐", "❤️", "🎉", "🫡",
];

export function EditorToolbar({
  areaRef,
  onChange,
  children,
}: {
  /** 受控 textarea 的 ref：插入动作直接操作 selection */
  areaRef: RefObject<HTMLTextAreaElement | null>;
  /** 插入后回写受控 value */
  onChange: (next: string) => void;
  /** 尾部扩展位：引用种子/视频插入行等既有插入器挂这里 */
  children?: ReactNode;
}) {
  const emojiOpen = useRef<HTMLDetailsElement>(null);
  // 工具栏提示与占位文案（2026-11-13 收编 i18n 门禁，四语同步；
  // ja 段回落 zh-CN）
  const et = useI18n().dict.forums3;

  function wrap(before: string, after: string, placeholder: string) {
    const el = areaRef.current;
    if (!el) return;
    const { selectionStart: s, selectionEnd: e, value } = el;
    const sel = value.slice(s, e) || placeholder;
    const next = value.slice(0, s) + before + sel + after + value.slice(e);
    onChange(next);
    requestAnimationFrame(() => {
      el.focus();
      const inner = before.length;
      el.setSelectionRange(s + inner, s + inner + sel.length);
    });
  }

  function insert(text: string, selectPlaceholder?: string) {
    const el = areaRef.current;
    if (!el) return;
    const { selectionStart: s, selectionEnd: e, value } = el;
    const body = selectPlaceholder ?? text;
    const next = value.slice(0, s) + body + value.slice(e);
    onChange(next);
    requestAnimationFrame(() => {
      el.focus();
      if (selectPlaceholder != null) {
        el.setSelectionRange(s, s + body.length);
      } else {
        el.setSelectionRange(s + text.length, s + text.length);
      }
    });
  }

  function linePrefix(prefix: string) {
    const el = areaRef.current;
    if (!el) return;
    const { selectionStart: s, selectionEnd: e, value } = el;
    // 选区所在行（首行行首到末行行尾）
    const ls = value.lastIndexOf("\n", s - 1) + 1;
    const nl = value.indexOf("\n", e);
    const le = nl === -1 ? value.length : nl;
    const block = value.slice(ls, le);
    const prefixed = block
      .split("\n")
      .map((l) => (l.startsWith(prefix) ? l : prefix + l))
      .join("\n");
    const next = value.slice(0, ls) + prefixed + value.slice(le);
    onChange(next);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(ls + prefixed.length, ls + prefixed.length);
    });
  }

  return (
    <div className="tp-ed__bar">
      <button type="button" className={BTN} title={et.etTitle}
        onClick={() => linePrefix("## ")}>
        H2
      </button>
      <button type="button" className={BTN} title={et.etBold}
        onClick={() => wrap("**", "**", et.etPhBold)}>
        <b>B</b>
      </button>
      <button type="button" className={BTN} title={et.etItalic}
        onClick={() => wrap("*", "*", et.etPhItalic)}>
        <i>I</i>
      </button>
      <button type="button" className={BTN} title={et.etStrike}
        onClick={() => wrap("~~", "~~", et.etPhStrike)}>
        <s>S</s>
      </button>
      <button type="button" className={BTN} title={et.etCode}
        onClick={() => wrap("`", "`", "code")}>
        {"<>"}
      </button>
      <button type="button" className={BTN} title={et.etCodeBlock}
        onClick={() =>
          insert("\n```\n" + et.etPhCode + "\n```\n", et.etPhCode)}>
        {"{}"}
      </button>
      <button type="button" className={BTN} title={et.etQuote}
        onClick={() => linePrefix("> ")}>
        ❝
      </button>
      <button type="button" className={BTN} title={et.etLink}
        onClick={() => wrap("[", "](https://)", et.etPhLink)}>
        🔗
      </button>
      <button type="button" className={BTN} title={et.etMention}
        onClick={() => insert(et.etPhMention, "@")}>
        @
      </button>
      <button type="button" className={BTN} title={et.etList}
        onClick={() => linePrefix("- ")}>
        ≡
      </button>
      <details
        ref={emojiOpen}
        className="tp-ed__emoji"
        onMouseLeave={() => emojiOpen.current?.removeAttribute("open")}
      >
        <summary className={BTN} title={et.etEmoji}>😀</summary>
        <div className="tp-ed__emoji-pop">
          {EMOJIS.map((e) => (
            <button
              key={e}
              type="button"
              className="tp-ed__emoji-btn"
              onClick={() => {
                insert(e);
                emojiOpen.current?.removeAttribute("open");
              }}
            >
              {e}
            </button>
          ))}
        </div>
      </details>
      {children}
    </div>
  );
}
