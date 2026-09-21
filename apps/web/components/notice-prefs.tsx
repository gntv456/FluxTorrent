"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 通知偏好开关（0075：GET/POST /me/notice-prefs，9 个白名单键，缺省 = 开） */
export function NoticePrefsCard() {
  const { dict } = useI18n();
  const t = dict.noticePrefs;
  const keys = Object.keys(t.keys);
  const [prefs, setPrefs] = useState<Record<string, boolean> | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const p = await api.get<Record<string, unknown>>(
        "/api/v1/me/notice-prefs",
      );
      // 后端是稀疏 JSON（缺省键 = 开）：显式为 false 才视为关
      const norm: Record<string, boolean> = {};
      for (const k of keys) norm[k] = p?.[k] !== false;
      setPrefs(norm);
    } catch {
      setPrefs(Object.fromEntries(keys.map((k) => [k, true])));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function toggle(key: string, enabled: boolean) {
    setBusy(key);
    setMsg(null);
    try {
      await api.post("/api/v1/me/notice-prefs", { key, enabled });
      setPrefs((p) => (p ? { ...p, [key]: enabled } : p));
      setMsg(t.saved);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.saveFailed);
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="baozi-panel flex flex-col gap-2 p-4">
      <h3 className="text-base font-bold">{t.title}</h3>
      <p className="text-xs text-sub">{t.note}</p>
      <ul className="grid grid-cols-1 gap-1 sm:grid-cols-2">
        {keys.map((k) => (
          <li
            key={k}
            className="flex items-center justify-between gap-2 rounded-[var(--r-sm)] border border-line px-3 py-1.5 text-sm"
          >
            <span>{t.keys[k]}</span>
            <button
              type="button"
              role="switch"
              aria-checked={prefs?.[k] ?? true}
              disabled={busy === k || prefs === null}
              onClick={() => toggle(k, !(prefs?.[k] ?? true))}
              className={`relative h-[22px] w-[44px] shrink-0 rounded-full transition ${
                (prefs?.[k] ?? true)
                  ? "bg-mint"
                  : "bg-[var(--surface-raised)] border border-line"
              }`}
            >
              <span
                className={`absolute top-[2px] h-[18px] w-[18px] rounded-full bg-white shadow transition-all ${
                  (prefs?.[k] ?? true) ? "left-[23px]" : "left-[2px]"
                }`}
              />
            </button>
          </li>
        ))}
      </ul>
      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}
    </section>
  );
}
