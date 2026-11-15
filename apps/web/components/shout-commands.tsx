"use client";

/** 命令提示（GET /shoutbox/bot）：占位 + 快捷命令按钮 */
export interface BotCommand {
  cmd: string;
  desc: string;
}

const cmdBtn =
  "flex min-h-11 items-center gap-2 rounded-lg " +
  "border border-line px-2 text-left text-xs " +
  "hover:border-[var(--sky)] disabled:opacity-50";

/** 机器人快捷命令按钮墙：点 = 直接发送执行（/me 动作除外——
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
  return (
    <div className="flex flex-col gap-1 px-3 pb-1">
      {commands.map((c) => (
        <button
          key={c.cmd}
          type="button"
          disabled={busy}
          onClick={() => {
            if (c.cmd.startsWith("/me ")) onFill(`${c.cmd} `);
            else onRun(c.cmd);
          }}
          className={cmdBtn}
        >
          <b className="shrink-0 text-[var(--sky)]">{c.cmd}</b>
          <span className="min-w-0 flex-1 truncate text-sub">
            {c.desc}
          </span>
        </button>
      ))}
    </div>
  );
}
