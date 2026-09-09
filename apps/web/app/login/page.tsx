"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { api, setSessionCookie, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

interface LoginResp {
  token: string;
  must_reset_password: boolean;
  user: { id: number; username: string; class_id: number };
}

/** 登录表单（useSearchParams 需 Suspense 包裹以满足静态预渲染） */
function LoginForm() {
  const { dict } = useI18n();
  const router = useRouter();
  const search = useSearchParams();
  const next = search.get("next") ?? "/torrents";
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // middleware 已挡未登录；这里兜底：残留会话 cookie 直接跳回目标页
  useEffect(() => {
    if (typeof window !== "undefined" && localStorage.getItem("flux.token")) {
      router.replace(next);
    }
  }, [router, next]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const resp = await api.post<LoginResp>("/api/v1/auth/login", {
        username,
        password,
      });
      localStorage.setItem("flux.token", resp.token);
      setSessionCookie(resp.token);
      router.push(next);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(
          dict.errors[err.code] ?? fmt(dict.login.fail, { code: err.code }),
        );
      } else {
        setError(dict.common.networkError);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} className="flex w-full flex-col gap-3">
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.login.username}</span>
        <input
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          required
          className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-white px-3 outline-none focus:ring-2 focus:ring-sky/40"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.login.password}</span>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          required
          className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-white px-3 outline-none focus:ring-2 focus:ring-sky/40"
        />
      </label>
      {error && (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      )}
      <button
        type="submit"
        disabled={busy}
        className="min-h-[44px] rounded-full bg-sky font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {busy ? dict.login.busy : dict.login.submit}
      </button>
    </form>
  );
}

/** 登录页（设计稿：吉祥物 + 蜡笔字标题；错误按 code 映射文案 §8.2） */
export default function LoginPage() {
  const { dict } = useI18n();
  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-12">
      <span aria-hidden className="text-[80px] leading-none">
        🦉
      </span>
      <h1 className="font-display text-3xl">{dict.login.welcome}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.login.subtitle}</p>
      <Suspense fallback={<div className="h-[220px]" aria-hidden />}>
        <LoginForm />
      </Suspense>
      <p className="text-xs text-sub">{dict.login.inviteOnly}</p>
    </div>
  );
}
