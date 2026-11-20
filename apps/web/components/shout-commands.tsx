"use client";

import { useEffect, useRef, useState } from "react";
import { useI18n } from "@/i18n/client";

/** 命令候选弹出面板（2026-11-16 自按钮墙收编）：原「每条命令一行常驻
 *  在输入框上方」占掉一整块版面（N 条命令 = N 行），改为与快捷表情同款
 *  交互——输入行里一枚「命令」触发钮，点开候选面板，选完即关。
 *  面板空（bot 未返回命令/拉取失败）时整枚钮不渲染。 */
export interface BotCommand {
  cmd: string;
  desc: string;
}

/** 机器人命令候选面板：点条目 = 直接发送执行（/me 动作除外——
 *  它要后续参数，填进输入框待补，shout-box 侧传 onFill 区分） */
export function ShoutCommands({
  commands,
  busy,
  onRun,
  onFill,
}: {
  commands: BotCommand[];
  busy: boolean;
  onRun: (cmd: string) => void;
  /** 需要用户补参数的命令（/me 动作）：填输入框 */
  onFill: (cmd: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.shoutbot;
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  // 点面板外收起（与 shout-emoji 同款交互）
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
        aria-label={t.commandsTitle}
        title={t.commandsTitle}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className={
          "flex h-11 shrink-0 items-center gap-0.5 rounded-lg border " +
          "border-line px-2 text-sm font-bold text-[var(--sky-text)] " +
          "hover:border-[var(--sky)] disabled:opacity-50"
        }
      >
        /
      </button>
      {open && (
        <div
          role="menu"
          aria-label={t.commandsTitle}
          className={
            "absolute bottom-full left-0 z-30 mb-1 flex w-max " +
            "max-w-[300px] flex-col gap-0.5 rounded-xl border " +
            "border-line bg-[var(--compat-paper)] p-2 shadow-lg"
          }
        >
          {commands.map((c) => (
            <button
              key={c.cmd}
              type="button"
              role="menuitem"
              disabled={busy}
              onClick={() => {
                setOpen(false);
                if (c.cmd.startsWith("/me ")) onFill(`${c.cmd} `);
                else onRun(c.cmd);
              }}
              className={
                "flex min-h-11 items-center gap-2 rounded-lg border " +
                "border-line px-2 text-left text-xs " +
                "hover:border-[var(--sky)] disabled:opacity-50"
              }
            >
              <b className="shrink-0 text-[var(--sky-text)]">{c.cmd}</b>
              <span className="min-w-0 flex-1 truncate text-sub">
                {c.desc}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
