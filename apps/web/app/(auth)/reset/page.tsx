"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";

import { Suspense, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import {
  api,
  setSessionCookie,
  hasSessionCookie,
  ApiError,
} from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 重置表单（useSearchParams 需 Suspense 包裹） */
function ResetForm() {
  const { dict } = useI18n();
  const router = useRouter();
  const search = useSearchParams();
  const token = search.get("token") ?? "";
  const [newPassword, setNewPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // 兜底：已登录访问重置页直接回资源库
  useEffect(() => {
    if (hasSessionCookie()) router.replace("/torrents");
  }, [router]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setOkMsg(null);
    if (newPassword !== confirm) {
      setError(dict.reset.mismatch);
      return;
    }
    setBusy(true);
    try {
      await api.post("/api/v1/auth/password/reset", {
        token: token.trim(),
        new_password: newPassword,
      });
      setOkMsg(dict.reset.success);
      // 2 秒后跳登录页用新密码登录
      setTimeout(() => router.push("/login"), 2000);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(dict.errors[err.code] ?? err.message);
      } else {
        setError(dict.common.networkError);
      }
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    INPUT_LG;

  if (!token) {
    return (
      <div className="flex w-full flex-col items-center gap-3">
        <p role="alert" className="text-sm text-danger">
          {dict.reset.noToken}
        </p>
        <Link href="/forgot" className="font-bold text-sky">
          {dict.reset.backToForgot}
        </Link>
      </div>
    );
  }

  return (
    <form onSubmit={submit} className="flex w-full flex-col gap-3">
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.reset.token}</span>
        <input
          value={token}
          readOnly
          className={`${inputCls} num bg-cloud text-xs`}
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.reset.newPassword}</span>
        <input
          type="password"
          value={newPassword}
          onChange={(e) => setNewPassword(e.target.value)}
          autoComplete="new-password"
          required
          minLength={8}
          className={inputCls}
        />
        <span className="text-xs text-sub">{dict.reset.passwordHint}</span>
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.reset.confirmPassword}</span>
        <input
          type="password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          autoComplete="new-password"
          required
          minLength={8}
          className={inputCls}
        />
      </label>
      {error && (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      )}
      {okMsg && (
        <p role="status" className="text-sm text-mint">
          {okMsg}
        </p>
      )}
      <button
        type="submit"
        disabled={busy}
        className={BTN_LG_SKY}
      >
        {busy ? dict.reset.busy : dict.reset.submit}
      </button>
    </form>
  );
}

/** 密码重置落地页：/reset?token=…（邮件链接直达，对接 POST /auth/password/reset） */
export default function ResetPage() {
  const { dict } = useI18n();
  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-8">
      <h1 className="font-display text-2xl">{dict.reset.title}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.reset.subtitle}</p>
      <Suspense fallback={<div className="h-[280px]" aria-hidden />}>
        <ResetForm />
      </Suspense>
      <p className="text-sm text-sub">
        {dict.reset.remembered}{" "}
        <Link href="/login" className="font-bold text-sky">
          {dict.reset.backToLogin}
        </Link>
      </p>
    </div>
  );
}
