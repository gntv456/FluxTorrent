"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";

import { useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 忘记密码：邮箱申请重置（防账号枚举：无论命中与否返回相同提示） */
export default function ForgotPage() {
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
        "/api/v1/auth/password/forgot",
        {
          email: email.trim(),
        },
      );
      setMsg(r.message);
    } catch (err) {
      if (err instanceof ApiError) {
        setMsg(dict.errors[err.code] ?? err.message);
      } else {
        setMsg(dict.common.networkError);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-8">
      <h1 className="font-display text-2xl">{dict.forgot.title}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.forgot.subtitle}</p>
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
          {busy ? dict.forgot.busy : dict.forgot.submit}
        </button>
      </form>
      <p className="text-xs text-sub">{dict.forgot.note}</p>
      <p className="text-sm text-sub">
        {dict.forgot.remembered}{" "}
        <Link href="/login" className="font-bold text-sky">
          {dict.forgot.backToLogin}
        </Link>
      </p>
    </div>
  );
}
