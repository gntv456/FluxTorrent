"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";

import { useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { AuthCanvas } from "@/components/auth-canvas";

/** 忘记密码：邮箱申请重置（防账号枚举：无论命中与否返回相同提示）。
 *  2026-10-18 对齐样图 § 账户与认证：改用认证画布（左深色 Hero + 右白卡），
 *  安全说明升级为卡内提示条。 */
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
        setMsg(apiErrorMessage(dict, err));
      } else {
        setMsg(dict.common.networkError);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <AuthCanvas
      title={dict.forgot.title}
      desc={dict.forgot.subtitle}
      heroTag={dict.forgot.heroTag}
      heroDesc={dict.forgot.heroDesc}
      notice={dict.forgot.note}
    >
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
          <span className="text-xs text-sub">{dict.forgot.emailHint}</span>
        </label>
        {msg && (
          <p role="status" className="text-sm text-sub">
            {msg}
          </p>
        )}
        <button type="submit" disabled={busy} className={BTN_LG_SKY}>
          {busy ? dict.forgot.busy : dict.forgot.submit}
        </button>
      </form>
      <div className="auth-card__links">
        <span>
          {dict.forgot.remembered}{" "}
          <Link href="/login">{dict.forgot.backToLogin}</Link>
        </span>
      </div>
    </AuthCanvas>
  );
}
