"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface Shout {
  id: number;
  username: string | null;
  message: string;
  created_at: string;
}

/** 机器人回话（仅本会话内展示，不落库——避免系统消息刷屏） */
interface BotReply {
  key: number;
  cmd: string;
  reply: string;
  at: string;
}

/** 命令提示（GET /shoutbox/bot）：占位 + 快捷命令按钮 */
interface BotCommand {
  cmd: string;
  desc: string;
}

/** 聊天盒（shoutbox.php 复刻）：最近 50 条 + 发言框（1-300 字），15s 自动刷新
 *  0078：消息以 / 开头时额外调用 GET /shoutbox/bot/exec?cmd= 并把回话插入聊天流 */
export function ShoutBox() {
  const { dict } = useI18n();
  const t = dict.shoutbox;
  const tb = dict.shoutbot;
  const [items, setItems] = useState<Shout[] | null>(null);
  const [text, setText] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [botReplies, setBotReplies] = useState<BotReply[]>([]);
  const [commands, setCommands] = useState<BotCommand[]>([]);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);
  const replySeq = useRef(0);

  const load = useCallback(async () => {
    try {
      setItems(await api.get<Shout[]>("/api/v1/shoutbox"));
    } catch {
      setItems([]);
    }
  }, []);
  useEffect(() => {
    load();
    timer.current = setInterval(load, 15000);
    return () => {
      if (timer.current) clearInterval(timer.current);
    };
  }, [load]);

  // 命令列表（占位提示 + 快捷按钮）；失败静默
  useEffect(() => {
    api
      .get<{ commands: BotCommand[] }>("/api/v1/shoutbox/bot")
      .then((r) => setCommands(r.commands ?? []))
      .catch(() => {});
  }, []);

  const botHint = commands.length
    ? tb.commandsHint.replace("{cmds}", commands.map((c) => c.cmd).join(" "))
    : "";

  async function send() {
    const raw = text.trim();
    if (!raw) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/shoutbox", { message: raw });
      setText("");
      // /命令：额外触发机器人回话（GET /shoutbox/bot/exec?cmd=），展示在本会话聊天流
      if (raw.startsWith("/")) {
        try {
          const r = await api.get<{ cmd: string; reply: string }>(
            `/api/v1/shoutbox/bot/exec?cmd=${encodeURIComponent(raw.split(/\s+/)[0])}`,
          );
          replySeq.current += 1;
          setBotReplies((list) =>
            [
              ...list,
              {
                key: replySeq.current,
                cmd: r.cmd,
                reply: r.reply,
                at: new Date().toISOString(),
              },
            ].slice(-10),
          );
        } catch (e) {
          setMsg(e instanceof ApiError ? e.message : tb.replyFailed);
        }
      }
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const timeStr = (iso: string) =>
    new Date(iso).toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });

  return (
    <section className="baozi-panel shoutbox">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">💬</span> {t.title}
        </h2>
        <small>{t.note}</small>
      </header>
      <ul className="shoutbox__list">
        {(items ?? [])
          .slice()
          .reverse()
          .map((s) => (
            <li key={s.id}>
              <b className="rainbow">{s.username ?? "?"}</b>
              <span className="shoutbox__text">{s.message}</span>
              <time>{timeStr(s.created_at)}</time>
            </li>
          ))}
        {botReplies.map((r) => (
          <li key={`bot-${r.key}`} className="shoutbox__bot">
            <b className="text-sky">🤖 {tb.botName}</b>
            <span className="shoutbox__text">{r.reply}</span>
            <time>{timeStr(r.at)}</time>
          </li>
        ))}
        {items !== null && items.length === 0 && botReplies.length === 0 && (
          <li className="shoutbox__empty">{t.empty}</li>
        )}
      </ul>
      {commands.length > 0 && (
        <div className="flex flex-wrap gap-1 px-3 pb-1">
          {commands.map((c) => (
            <button
              key={c.cmd}
              type="button"
              title={c.desc}
              onClick={() => setText(c.cmd)}
              className="min-h-[26px] rounded-full border border-line px-2 text-[11px] font-bold text-sub hover:border-sky hover:text-sky"
            >
              {c.cmd}
            </button>
          ))}
        </div>
      )}
      {msg && <p className="shoutbox__msg">{msg}</p>}
      <form
        className="shoutbox__form"
        onSubmit={(e) => {
          e.preventDefault();
          void send();
        }}
      >
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          maxLength={300}
          placeholder={botHint || t.placeholder}
          aria-label={t.title}
        />
        <button
          type="submit"
          className="baozi-button"
          disabled={busy || !text.trim()}
        >
          {t.send}
        </button>
      </form>
    </section>
  );
}
