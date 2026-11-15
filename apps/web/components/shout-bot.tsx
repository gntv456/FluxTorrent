"use client";

import { useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import type { BotReply } from "@/components/shout-row";
import type { FreeCard } from "@/components/shout-free-cards";

/** 机器人回话会话态（0078）：发 /命令 时调 GET /shoutbox/bot/exec?cmd=，
 *  回话插进本会话聊天流（不落库——避免系统消息刷屏），至多留 10 条。
 *  从 shout-box 抽出：发送侧只 await exec(raw) 一行。 */
export function useBotReplies() {
  const [replies, setReplies] = useState<BotReply[]>([]);
  const seq = useRef(0);

  /** 发送一条机器人命令；失败抛 ApiError 由调用侧统一提示 */
  async function exec(raw: string) {
    const head = raw.split(/\s+/)[0];
    const r = await api.get<{
      cmd: string;
      reply: string;
      cards?: FreeCard[];
    }>(
      `/api/v1/shoutbox/bot/exec?cmd=${encodeURIComponent(head)}`,
    );
    seq.current += 1;
    setReplies((list) =>
      [
        ...list,
        {
          key: seq.current,
          cmd: r.cmd,
          reply: r.reply,
          cards: r.cards,
          at: new Date().toISOString(),
        },
      ].slice(-10),
    );
  }

  /** 清空面板（职员 clear 配套） */
  const reset = () => setReplies([]);
  const isApiErr = (e: unknown) => e instanceof ApiError;
  return { replies, exec, reset, isApiErr };
}
