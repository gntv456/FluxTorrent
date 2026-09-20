"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { api, setSessionCookie, hasSessionCookie, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 登录表单域（从 app/login/page.tsx 按域拆出，再从 login-view 细分）：
 *  SVG 图标 + 登录表单本体（错误展示/剩余尝试/2FA 展开/辅助链接卡）。 */

interface LoginResp {
  token: string;
  must_reset_password: boolean;
  user: { id: number; username: string; class_id: number };
}

/** SVG 图标（theme.css：stroke 1.8 / round，25×25 左侧嵌入输入框） */
export function IconUser() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden>
      <circle cx="12" cy="8" r="3.6" />
      <path d="M4.8 20c1.2-3.6 4-5.4 7.2-5.4s6 1.8 7.2 5.4" />
    </svg>
  );
}
export function IconLock() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden>
      <rect x="5" y="10.5" width="14" height="9" rx="2" />
      <path d="M8.2 10.5V8a3.8 3.8 0 0 1 7.6 0v2.5" />
      <circle cx="12" cy="15" r="1.3" />
    </svg>
  );
}
export function IconShield() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden>
      <path d="M12 3.5 19 6v5.4c0 4.4-2.9 7.6-7 9.1-4.1-1.5-7-4.7-7-9.1V6z" />
      <path d="m9.2 12 2 2 3.6-3.8" />
    </svg>
  );
}
export function IconFinger() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden>
      <path d="M7 11.5a5 5 0 0 1 10 0v2.2" />
      <path d="M10 11.5a2 2 0 0 1 4 0v4.6" />
      <path d="M12 18.5v.5" />
      <path d="M4.5 11.5a7.5 7.5 0 0 1 15 0" />
    </svg>
  );
}

/** 登录表单（useSearchParams 需 Suspense 包裹以满足静态预渲染） */
export function LoginForm() {
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
  const [showTotp, setShowTotp] = useState(false);
  const [showPwd, setShowPwd] = useState(false);

  // middleware 已挡未登录；这里兜底：残留会话直接跳回目标页。
  // P1 收敛后 token 为 HttpOnly（JS 不可读），登录态以 flux.session 标记判定；
  // session(12h) 早于 flux_token(24h) 过期时由后端 401 → api-client 全局清理跳转。
  useEffect(() => {
    if (typeof window === "undefined") return;
    if (hasSessionCookie()) router.replace(next);
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
      setSessionCookie(true);
      // 临时密码（P1 修复）：后端只放行改密/登出三条路径，落在业务页只会
      // 处处报「请先修改密码」——登录成功即引导直达「我的 → 安全」改密。
      router.push(resp.must_reset_password ? "/my?tab=security" : next);
    } catch (err) {
      if (err instanceof ApiError) {
        // 展示优先级：字典文案（三语）→ 后端 message（兜底，含具体原因）→ 通用失败
        // 不再显示「登录失败（数字码）」——用户看不懂错误码，必须给文字原因
        setError(
          dict.errors[err.code] ?? err.message ?? fmt(dict.login.fail, { code: err.code }),
        );
        if (err.code === 2004) setAttempts((n) => n + 1);
        // 2FA：密码已对但缺/错验证码 —— 展开验证码输入框并聚焦，让用户立刻知道下一步
        if (err.code === 2010 || err.code === 2011) {
          setShowTotp(true);
        }
      } else {
        setError(dict.common.networkError);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} className="bz-form">
      {/* 安全校验提示（视觉隐藏，aria 可达） */}
      <p className="bz-visually-hidden">{dict.login.securityNote}</p>

      {error && (
        <p role="alert" className="bz-form-error">
          {error}
        </p>
      )}
      {/* 剩余尝试提醒（后端 §5.7：5 次/分钟/用户名） */}
      {attempts > 0 && attempts < 5 && (
        <p className="bz-form-error bz-form-error--soft">
          {fmt(dict.login.remainingTries, { n: 5 - attempts })}
        </p>
      )}

      <div className="bz-fields">
        <label className="bz-field">
          <span className="bz-field-icon">
            <IconUser />
          </span>
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            required
            placeholder={dict.login.username}
          />
        </label>
        <label className="bz-field">
          <span className="bz-field-icon">
            <IconLock />
          </span>
          <input
            type={showPwd ? "text" : "password"}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            required
            placeholder={dict.login.password}
          />
          <button
            type="button"
            className="bz-pwd-toggle"
            aria-pressed={showPwd}
            aria-label={dict.login.password}
            onClick={() => setShowPwd((v) => !v)}
          >
            <svg viewBox="0 0 24 24" aria-hidden>
              <path d="M3 12s3.5-6 9-6 9 6 9 6-3.5 6-9 6-9-6-9-6z" />
              <circle cx="12" cy="12" r="2.6" />
              {!showPwd && <path d="M5 5l14 14" className="bz-toggle-slash" />}
            </svg>
          </button>
        </label>
        {showTotp && (
          <label className="bz-field">
            <span className="bz-field-icon">
              <IconShield />
            </span>
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]*"
              maxLength={6}
              value={totp}
              onChange={(e) => setTotp(e.target.value.replace(/\D/g, ""))}
              placeholder={dict.login.twoStepCode}
            />
          </label>
        )}
      </div>

      {/* 记住登录 + 两步验证开关 行（theme.css .auth-form-options 双列网格） */}
      <div className="bz-options">
        <label className="bz-remember">
          <input type="checkbox" defaultChecked />
          <span className="bz-checkbox-mark" aria-hidden />
          {dict.login.remember}
        </label>
        <button
          type="button"
          onClick={() => setShowTotp((v) => !v)}
          aria-expanded={showTotp}
          className="bz-totp-toggle"
        >
          {dict.login.twoStepCode}
        </button>
      </div>

      <div className="bz-actions">
        <button type="submit" disabled={busy} className="bz-submit">
          {busy ? dict.login.busy : dict.login.submit}
        </button>
      </div>

      {/* 账号辅助链接卡（NexusPHP login.php 全口径：菱形符号 + 虚线分隔） */}
      <div className="bz-account-links">
        <div className="bz-assistance">
          <p>
            {dict.login.noAccount}
            <Link href="/register">{dict.login.signup}</Link>
          </p>
          <p>
            {dict.login.forgotPassword}
            <Link href="/forgot">{dict.login.forgotLink}</Link>
          </p>
          <p>
            {dict.login.bannedIntro}
            <Link href="/ban-log">{dict.login.bannedLog}</Link>
          </p>
          <p>
            {dict.login.resendIntro}
            <Link href="/resend">{dict.login.resendLink}</Link>
          </p>
          <p>
            {dict.login.appealIntro}
            <Link href="/appeals">{dict.login.appealLink}</Link>
          </p>
        </div>
      </div>

      {/* 其他登录方式（虚线上分隔的方法 chips） */}
      <div className="bz-methods" aria-label={dict.login.ssoLabel}>
        <button type="button" className="bz-method" disabled title={dict.login.passkeySoon}>
          <IconFinger />
          <span>{dict.login.passkey}</span>
        </button>
      </div>
    </form>
  );
}
