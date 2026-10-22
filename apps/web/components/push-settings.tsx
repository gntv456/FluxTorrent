"use client";

import { BTN_SM_DANGER } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { pushTestMessage, type PushTestTally } from "@/lib/push-test-result";
import { useI18n } from "@/i18n/client";

/** 浏览器推送订阅开关（控制面板 · 安全设定内嵌）
 *  后端：GET /push/vapid-key → POST /push/subscribe → POST /push/test
 *  P-574：send 侧按 notice_prefs 的 push_hr/push_promo/push_message 逐类
 *  过滤，此前全站无任何写入口 —— 订阅了就只能全收。这里补三个开关。 */
const PUSH_KEYS = ["push_hr", "push_promo", "push_message"] as const;
type PushKey = (typeof PUSH_KEYS)[number];
const SWITCH_BASE =
  "relative h-[22px] w-[44px] shrink-0 rounded-full transition";
const SWITCH_KNOB =
  "absolute top-[2px] h-[18px] w-[18px] rounded-full bg-white " +
  "shadow transition-all";

export function PushSettings() {
  const { dict } = useI18n();
  const t = dict.push2;
  const [enabled, setEnabled] = useState<boolean | null>(null); // 服务端 VAPID 是否配置
  const [subscribed, setSubscribed] = useState(false);
  const [prefs, setPrefs] = useState<Record<PushKey, boolean> | null>(null);
  const [prefBusy, setPrefBusy] = useState<PushKey | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .get<{ public_key: string; enabled: boolean }>("/api/v1/push/vapid-key")
      .then((r) => setEnabled(r.enabled))
      .catch(() => setEnabled(false));
    api
      .get<Record<string, unknown>>("/api/v1/me/notice-prefs")
      .then((p) => {
        const on = (k: PushKey) => (p?.[k] !== false) as boolean;
        setPrefs({
          push_hr: on("push_hr"),
          push_promo: on("push_promo"),
          push_message: on("push_message"),
        });
      })
      .catch(() => setPrefs(null));
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

  async function togglePref(key: PushKey, on: boolean) {
    setPrefBusy(key);
    try {
      await api.post("/api/v1/me/notice-prefs", { key, enabled: on });
      setPrefs((p) => (p ? { ...p, [key]: on } : p));
      flash(t.prefSaved);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setPrefBusy(null);
    }
  }

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
        // P-370 的后半：显式话题会覆盖后端 default_topics，
        // 漏 "hr" 时 H&R 预警/违规推送结构上永远送不到本订阅。
        topics: ["promo", "request", "message", "hr"],
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
      const r = await api.post<PushTestTally>("/api/v1/push/test", {});
      flash(pushTestMessage(r, t));
      // 服务端已把这枚订阅标成失效：本地开关也不能再演「已订阅」。
      if ((r.delivered ?? 0) === 0 && (r.expired_cleaned ?? 0) > 0) {
        setSubscribed(false);
      }
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="brand-panel flex flex-col gap-3 p-4">
      <h3 className="text-base font-bold text-ink">{t.title}</h3>
      {msg && <p className="text-xs text-[var(--brand-orange-dark)]">{msg}</p>}
      {enabled === false ? (
        <p className="text-xs text-sub">{t.serverDisabled}</p>
      ) : (
        <>
          <p className="text-xs text-sub">
            {subscribed ? t.statusOn : t.statusOff} · {t.topicsNote}
          </p>
          {prefs && (
            <ul className="flex flex-col gap-1">
              {PUSH_KEYS.map((k) => {
                const on = prefs[k];
                return (
                  <li
                    key={k}
                    className="flex items-center justify-between gap-2"
                  >
                    <span className="text-xs">{t.prefs[k]}</span>
                    <button
                      type="button"
                      role="switch"
                      aria-label={t.prefs[k]}
                      aria-checked={on}
                      disabled={prefBusy === k}
                      onClick={() => togglePref(k, !on)}
                      className={`${SWITCH_BASE} ${
                        on
                          ? "bg-mint"
                          : "bg-[var(--surface-raised)] border border-line"
                      }`}
                    >
                      <span
                        className={`${SWITCH_KNOB} ${
                          on ? "left-[23px]" : "left-[2px]"
                        }`}
                      />
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
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
                  className={BTN_SM_DANGER}
                  disabled={busy}
                  onClick={unsubscribe}
                >
                  {t.btnUnsub}
                </button>
              </>
            ) : (
              <button
                className="brand-button self-start"
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
