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

/** 聊天盒（shoutbox.php 复刻）：最近 50 条 + 发言框（1-300 字），15s 自动刷新 */
export function ShoutBox() {
  const { dict } = useI18n();
  const t = dict.shoutbox;
  const [items, setItems] = useState<Shout[] | null>(null);
  const [text, setText] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

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

  async function send() {
    if (!text.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/shoutbox", { message: text.trim() });
      setText("");
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel shoutbox">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">💬</span> {t.title}
        </h2>
        <small>{t.note}</small>
      </header>
      <ul className="shoutbox__list">
        {(items ?? []).slice().reverse().map((s) => (
          <li key={s.id}>
            <b className="rainbow">{s.username ?? "?"}</b>
            <span className="shoutbox__text">{s.message}</span>
            <time>
              {new Date(s.created_at).toLocaleTimeString("zh-CN", {
                hour: "2-digit",
                minute: "2-digit",
              })}
            </time>
          </li>
        ))}
        {items !== null && items.length === 0 && (
          <li className="shoutbox__empty">{t.empty}</li>
        )}
      </ul>
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
          placeholder={t.placeholder}
          aria-label={t.title}
        />
        <button type="submit" className="baozi-button" disabled={busy || !text.trim()}>
          {t.send}
        </button>
      </form>
    </section>
  );
}
