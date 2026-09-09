"use client";

import { Suspense, useEffect, useState } from "react";
import Link from "next/link";
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
  const [totp, setTotp] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [attempts, setAttempts] = useState<number>(0); // 连续失败次数（前端口径，5 次触发后端限流）
  const [busy, setBusy] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);

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
        totp_code: totp.trim() ? Number(totp.trim()) : undefined,
      });
      localStorage.setItem("flux.token", resp.token);
      setSessionCookie(resp.token);
      router.push(next);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(
          dict.errors[err.code] ?? fmt(dict.login.fail, { code: err.code }),
        );
        if (err.code === 2004) setAttempts((n) => n + 1);
      } else {
        setError(dict.common.networkError);
      }
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[44px] rounded-[var(--r-sm)] border border-line bg-white px-3 outline-none focus:ring-2 focus:ring-sky/40";

  return (
    <form onSubmit={submit} className="flex w-full flex-col gap-3">
      <label className="flex flex-col gap-1">
        <span className="text-sm text-sub">{dict.login.username}</span>
        <input
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          required
          className={inputCls}
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
          className={inputCls}
        />
      </label>

      {/* 高级选项（NexusPHP 同款折叠）：两步验证码 */}
      <button
        type="button"
        onClick={() => setShowAdvanced((v) => !v)}
        aria-expanded={showAdvanced}
        className="self-start text-sm text-sky"
      >
        {showAdvanced ? "▾ " : "▸ "}
        {dict.login.advancedOptions}
      </button>
      {showAdvanced && (
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.login.twoStepCode}</span>
          <input
            type="text"
            inputMode="numeric"
            pattern="[0-9]*"
            maxLength={6}
            value={totp}
            onChange={(e) => setTotp(e.target.value.replace(/\D/g, ""))}
            placeholder={dict.login.twoStepTooltip}
            className={inputCls}
          />
        </label>
      )}

      {error && (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      )}
      {/* 剩余尝试提醒（后端 §5.7：5 次/分钟/用户名） */}
      {attempts > 0 && attempts < 5 && (
        <p className="text-xs text-sub">
          {fmt(dict.login.remainingTries, { n: 5 - attempts })}
        </p>
      )}

      <div className="flex gap-2">
        <button
          type="submit"
          disabled={busy}
          className="min-h-[44px] flex-1 rounded-full bg-sky font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          {busy ? dict.login.busy : dict.login.submit}
        </button>
        <button
          type="reset"
          onClick={() => {
            setUsername("");
            setPassword("");
            setTotp("");
            setError(null);
          }}
          className="min-h-[44px] rounded-full border border-line px-5 text-sm text-sub active:scale-[0.97]"
        >
          {dict.login.reset}
        </button>
      </div>
    </form>
  );
}

/** 登录页（设计稿：吉祥物 + 蜡笔字标题；字段对齐 NexusPHP login.php） */
export default function LoginPage() {
  const { dict } = useI18n();
  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-12">
      <span aria-hidden className="text-[80px] leading-none">
        🦉
      </span>
      <h1 className="font-display text-3xl">{dict.login.welcome}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.login.subtitle}</p>
      <Suspense fallback={<div className="h-[260px]" aria-hidden />}>
        <LoginForm />
      </Suspense>

      {/* 安全提示（NexusPHP：cookie 提示 + 失败封禁警示） */}
      <p className="text-xs leading-relaxed text-sub">
        {dict.login.cookieNote}
        <br />
        {fmt(dict.login.failBanNote, { n: 5 })}
      </p>

      {/* 账号辅助链接（NexusPHP：注册/找回密码） */}
      <div className="flex w-full flex-col gap-2 border-t border-line pt-4 text-sm">
        <p className="text-sub">
          {dict.login.noAccount}{" "}
          <Link href="/register" className="font-bold text-sky">
            {dict.login.signup}
          </Link>
        </p>
        <p className="text-sub">
          {dict.login.forgotPassword}{" "}
          <Link href="/forgot" className="font-bold text-sky">
            {dict.login.recoverByEmail}
          </Link>
        </p>
      </div>

      <p className="text-xs text-sub">{dict.login.inviteOnly}</p>
    </div>
  );
}
