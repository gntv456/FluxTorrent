"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 浏览器推送订阅开关（控制面板 · 安全设定内嵌）
 *  后端：GET /push/vapid-key → POST /push/subscribe → POST /push/test */
export function PushSettings() {
  const { dict } = useI18n();
  const t = dict.push2;
  const [enabled, setEnabled] = useState<boolean | null>(null); // 服务端 VAPID 是否配置
  const [subscribed, setSubscribed] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .get<{ public_key: string; enabled: boolean }>("/api/v1/push/vapid-key")
      .then((r) => setEnabled(r.enabled))
      .catch(() => setEnabled(false));
    // 本地是否已订阅（sw 注册 + localStorage 标记）
    if (typeof window !== "undefined" && "serviceWorker" in navigator) {
      navigator.serviceWorker
        .getRegistration()
        .then((reg) => reg?.pushManager.getSubscription())
        .then((sub) => setSubscribed(!!sub))
        .catch(() => {});
    }
  }, []);
  useEffect(load, [load]);

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 3500);
  }

  async function subscribe() {
    setBusy(true);
    try {
      const reg = await navigator.serviceWorker.ready;
      const vapid = await api.get<{ public_key: string }>(
        "/api/v1/push/vapid-key",
      );
      const key = urlB64ToBufferSource(vapid.public_key);
      const sub = await reg.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: key,
      });
      const j = sub.toJSON();
      await api.post("/api/v1/push/subscribe", {
        endpoint: j.endpoint,
        keys: j.keys,
        topics: ["promo", "request", "message"],
      });
      setSubscribed(true);
      flash(t.subscribed);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : t.browserUnsupported);
    } finally {
      setBusy(false);
    }
  }

  async function unsubscribe() {
    setBusy(true);
    try {
      const reg = await navigator.serviceWorker.ready;
      const sub = await reg.pushManager.getSubscription();
      if (sub) {
        await api.post("/api/v1/push/unsubscribe", { endpoint: sub.endpoint });
        await sub.unsubscribe();
      }
      setSubscribed(false);
      flash(t.unsubscribed);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function sendTest() {
    setBusy(true);
    try {
      await api.post("/api/v1/push/test", {});
      flash(t.testSent);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="baozi-panel flex flex-col gap-3 p-4">
      <h3 className="text-base font-bold text-ink">{t.title}</h3>
      {msg && <p className="text-xs text-[var(--baozi-orange-dark)]">{msg}</p>}
      {enabled === false ? (
        <p className="text-xs text-sub">{t.serverDisabled}</p>
      ) : (
        <>
          <p className="text-xs text-sub">
            {subscribed ? t.statusOn : t.statusOff} · {t.topicsNote}
          </p>
          <div className="flex flex-wrap gap-2">
            {subscribed ? (
              <>
                <button
                  className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
                  disabled={busy}
                  onClick={sendTest}
                >
                  {t.btnTest}
                </button>
                <button
                  className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger disabled:opacity-50"
                  disabled={busy}
                  onClick={unsubscribe}
                >
                  {t.btnUnsub}
                </button>
              </>
            ) : (
              <button
                className="baozi-button self-start"
                disabled={busy}
                onClick={subscribe}
              >
                {t.btnSub}
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function urlB64ToBufferSource(base64: string): ArrayBuffer {
  const padding = "=".repeat((4 - (base64.length % 4)) % 4);
  const b64 = (base64 + padding).replace(/-/g, "+").replace(/_/g, "/");
  const raw = atob(b64);
  const buf = new ArrayBuffer(raw.length);
  const view = new Uint8Array(buf);
  for (let i = 0; i < raw.length; i++) view[i] = raw.charCodeAt(i);
  return buf;
}
