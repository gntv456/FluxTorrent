"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";
import { Icon } from "@/components/icons";

import { useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 重发验证/通知邮件（NexusPHP confirm_resend.php 口径，公开；后端防枚举同响应） */
export default function ResendPage() {
  const { dict } = useI18n();
  const [email, setEmail] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ message: string }>(
        "/api/v1/auth/confirm/resend",
        {
          email: email.trim(),
        },
      );
      setMsg(r.message);
    } catch (err) {
      setMsg(
        err instanceof ApiError
          ? (dict.errors[err.code] ?? err.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-12">
      <Icon name="seed" size={72} className="text-[var(--sky)]" />
      <h1 className="font-display text-3xl">{dict.resend.title}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.resend.subtitle}</p>
      <form onSubmit={submit} className="flex w-full flex-col gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.forgot.email}</span>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="email"
            required
            className={INPUT_LG}
          />
        </label>
        {msg && (
          <p role="status" className="text-sm text-sub">
            {msg}
          </p>
        )}
        <button
          type="submit"
          disabled={busy}
          className={BTN_LG_SKY}
        >
          {busy ? dict.resend.busy : dict.resend.submit}
        </button>
      </form>
      <p className="text-xs text-sub">{dict.resend.note}</p>
      <p className="text-sm text-sub">
        <Link href="/login" className="font-bold text-sky">
          {dict.forgot.backToLogin}
        </Link>
      </p>
    </div>
  );
}
