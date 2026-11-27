"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import type { PvpMessage } from "./pvp-lobby";

interface MessagesResp {
  room_id: number;
  messages: PvpMessage[];
}

/** 房间频道消息：等待房内座位 + 观战可见；轮询追加，发言截断 120 字。 */
export function RoomChat({ roomId }: { roomId: number }) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const [messages, setMessages] = useState<PvpMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    try {
      const r = await api.get<MessagesResp>(
        `/api/v1/games/doudizhu/pvp/messages?room_id=${roomId}`,
      );
      setMessages(r.messages ?? []);
    } catch {
      /* 拉取失败不阻断房间 */
    }
  }, [roomId]);

  useEffect(() => {
    void load();
    const tick = window.setInterval(() => void load(), 5000);
    return () => window.clearInterval(tick);
  }, [load]);

  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages]);

  async function send() {
    const body = draft.trim().slice(0, 120);
    if (!body || sending) return;
    setSending(true);
    setErr(null);
    try {
      await api.post("/api/v1/games/doudizhu/pvp/messages", {
        room_id: roomId,
        body,
      });
      setDraft("");
      await load();
    } catch (e) {
      setErr(apiErrorMessage(dict, e));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="dz-chat">
      <div className="dz-chat-title">{t.chatTitle}</div>
      <div className="dz-chat-list" ref={listRef} aria-live="polite">
        {messages.length === 0 ? (
          <div className="dz-chat-empty">{t.chatEmpty}</div>
        ) : (
          messages.map((m) => (
            <div key={m.id} className="dz-chat-msg">
              <span className="dz-chat-name">{m.nickname}</span>
              <span className="dz-chat-body">{m.body}</span>
            </div>
          ))
        )}
      </div>
      <div className="dz-chat-input">
        <input
          value={draft}
          maxLength={120}
          placeholder={t.chatPlaceholder}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void send();
          }}
        />
        <button
          type="button"
          className="dz-btn"
          disabled={sending || !draft.trim()}
          onClick={() => void send()}
        >
          {t.chatSend}
        </button>
      </div>
      {err && <div className="dz-chat-err" role="alert">{err}</div>}
    </div>
  );
}
