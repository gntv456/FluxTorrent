"use client";

import { useI18n } from "@/i18n/client";
import { isStaffClass } from "@/lib/domain/user-class";
import type { FreeCard } from "@/components/shout-free-cards";
import { ShoutFreeCards } from "@/components/shout-free-cards";

/** 聊天消息行（列表 + 机器人回话两种形态），从 shout-box 抽出守行数门禁 */

export interface Shout {
  id: number;
  user_id: number;
  username: string | null;
  message: string;
  created_at: string;
}

/** 机器人回话（仅本会话内展示，不落库——避免系统消息刷屏） */
export interface BotReply {
  key: number;
  cmd: string;
  reply: string;
  /** /free 的免费种子卡片数据（0420 播报带种卡）；其余命令缺省 */
  cards?: FreeCard[];
  at: string;
}

const delBtn =
  "shrink-0 text-xs text-[var(--danger-text)] " +
  "underline disabled:opacity-50";

/** 普通消息行：/me 动作（`* ` 前缀）斜体第三人称；本人 2 分钟内 / 职员可删 */
export function ShoutRow({
  s,
  meId,
  staff,
  busy,
  onDel,
  timeStr,
}: {
  s: Shout;
  meId: number | null;
  staff: boolean;
  busy: boolean;
  onDel: (id: number) => void;
  timeStr: (iso: string) => string;
}) {
  const { dict } = useI18n();
  const t = dict.shoutbox;
  // /me 动作（后端存 `* ` 前缀）：斜体第三人称「* 名字 动作」
  const action = s.message.startsWith("* ")
    ? s.message.slice(2)
    : null;
  const own = meId !== null && s.user_id === meId;
  const recent = Date.now() - new Date(s.created_at).getTime() <= 120_000;
  const canDel = staff || (own && recent);
  return (
    <li>
      <b className="rainbow">{s.username ?? "?"}</b>
      <span
        className={
          action
            ? "shoutbox__text italic text-faint"
            : "shoutbox__text"
        }
      >
        {action ? `* ${s.username ?? "?"} ${action}` : s.message}
      </span>
      <time>{timeStr(s.created_at)}</time>
      {canDel && (
        <button
          type="button"
          aria-label={t.del}
          title={t.del}
          disabled={busy}
          onClick={() => onDel(s.id)}
          className={delBtn}
        >
          {t.del}
        </button>
      )}
    </li>
  );
}

/** 机器人回话行：蓝底 + 多行清单（pre-line）+ /free 种卡 */
export function ShoutBotRow({
  r,
  botName,
  timeStr,
}: {
  r: BotReply;
  botName: string;
  timeStr: (iso: string) => string;
}) {
  return (
    <li className="shoutbox__bot">
      <b className="text-[var(--sky)]">🤖 {botName}</b>
      {/* 纵列包一层：li 是 flex 行布局，种卡块要落在回话文本下方 */}
      <span className="flex min-w-0 flex-1 flex-col">
        {/* /help 清单是多行文本，pre-line 保住 \n 排版 */}
        <span className="shoutbox__text whitespace-pre-line">
          {r.reply}
        </span>
        {r.cards && r.cards.length > 0 && (
          <ShoutFreeCards cards={r.cards} />
        )}
      </span>
      <time>{timeStr(r.at)}</time>
    </li>
  );
}
