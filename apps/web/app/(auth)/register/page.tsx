"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";
import { Icon } from "@/components/icons";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api, setSessionCookie, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  DRIVER_WIDGET_ID,
  mountCaptchaWidget,
} from "@/lib/captcha-widget";
import { RegisterFields } from "@/components/register-fields";

interface Captcha {
  captcha_id: string;
  question: string;
}

/// register-mode 下发的公开配置（0227）：验证码驱动 + 申请通道
interface ModeInfo {
  mode?: string;
  captcha_provider?: string;
  captcha_site_key?: string;
  application_signup?: string;
}

/** 注册页（M01 邀请制）：邀请码 + 算术验证码 + 账号密码，对接 POST /auth/register */
export default function RegisterPage() {
  const { dict } = useI18n();
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [inviteCode, setInviteCode] = useState("");
  const [captchaAnswer, setCaptchaAnswer] = useState("");
  // 自定义字段（0186）：站长标记 show_on_register 的字段动态渲染
  const [regFields, setRegFields] = useState<
    {
      key: string;
      label: string;
      type: string;
      required: boolean;
      options: { value: string; label?: string }[];
    }[]
  >([]);
  const [fieldVals, setFieldVals] = useState<Record<string, unknown>>({});
  const [captcha, setCaptcha] = useState<Captcha | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 注册模式（0204）：open 模式不强制邀请码；invite_only 需要
  // （0208：email_verify 选项已移除，仅 invite_only/open 两态）
  const [openMode, setOpenMode] = useState(false);
  // 0227：验证码驱动（none=自研算术题，其他=第三方组件 token）
  const [driver, setDriver] = useState("none");
  const [siteKey, setSiteKey] = useState("");
  const [captchaToken, setCaptchaToken] = useState("");

  async function refreshCaptcha() {
    try {
      setCaptcha(await api.get<Captcha>("/api/v1/auth/captcha"));
    } catch {
      setCaptcha(null);
    }
  }

  useEffect(() => {
    // ?invite=xxx 预填（邮件邀请链接落地，0204）
    const q = new URLSearchParams(window.location.search);
    const inv = q.get("invite");
    if (inv) setInviteCode(inv);
    refreshCaptcha();
    fetch("/api/v1/register-fields")
      .then((r) => r.json())
      .then((b: { data?: typeof regFields }) =>
        setRegFields(b?.data ?? []),
      )
      .catch(() => setRegFields([]));
    // 注册模式与后端 registration_mode 同步（open 下邀请码非必填）
    api
      .get<ModeInfo>("/api/v1/register-mode")
      .then((r) => {
        setOpenMode(r.mode === "open");
        const prov = r.captcha_provider ?? "none";
        setDriver(prov);
        setSiteKey(r.captcha_site_key ?? "");
        mountCaptchaWidget(prov, r.captcha_site_key ?? "", setCaptchaToken);
      })
      .catch(() => setOpenMode(false));
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    setOkMsg(null);
    try {
      await api.post("/api/v1/auth/register", {
        username: username.trim(),
        email: email.trim(),
        password,
        invite_code: inviteCode.trim(),
        captcha_id: driver === "none" ? (captcha?.captcha_id ?? "") : "",
        captcha_answer:
          driver === "none" && captchaAnswer ? Number(captchaAnswer) : 0,
        captcha_token: captchaToken,
        fields: fieldVals,
      });
      // 注册成功直接走登录（后端注册不返回 token）
      const resp = await api.post<{ token: string }>("/api/v1/auth/login", {
        username: username.trim(),
        password,
      });
      setSessionCookie(true);
      router.push("/torrents");
    } catch (err) {
      if (err instanceof ApiError) {
        setMsg(dict.errors[err.code] ?? err.message);
      } else {
        setMsg(dict.common.networkError);
      }
      refreshCaptcha(); // 验证码一次性，失败即换新题
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    INPUT_LG;

  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-8">
      {/* M5.1：品牌头由 (auth) 壳统一渲染，页内只留标题 */}
      <h1 className="font-display text-2xl">{dict.register.title}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.register.subtitle}</p>
      <form onSubmit={submit} className="flex w-full flex-col gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">
            {dict.register.inviteCode}
            {openMode && (
              <span className="ml-1 font-normal text-fainter">
                {dict.register.inviteOptional}
              </span>
            )}
          </span>
          <input
            value={inviteCode}
            onChange={(e) => setInviteCode(e.target.value)}
            required={!openMode}
            maxLength={32}
            className={inputCls}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.register.username}</span>
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            required
            minLength={3}
            maxLength={24}
            className={inputCls}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.register.email}</span>
          {/* 定向邀请提示（2026-09-27）：邮件链接落地（?invite= 预填）时
              后端校验注册邮箱须与收到邀请的邮箱一致 */}
          {inviteCode && (
            <span className="text-xs text-[var(--text-faint)]">
              {dict.register.emailMustMatch}
            </span>
          )}
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="email"
            required
            className={inputCls}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.register.password}</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="new-password"
            required
            minLength={8}
            className={inputCls}
          />
          <span className="text-xs text-sub">{dict.register.passwordHint}</span>
        </label>
        {/* 验证码（0227）：none=自研算术题；第三方=对应组件（token 由回调写入） */}
        {driver !== "none" && siteKey ? (
          <div id={DRIVER_WIDGET_ID[driver]} className="min-h-[64px]" />
        ) : null}
        {/* 算术验证码（后端 /auth/captcha 出题，一次性） */}
        {driver === "none" && (
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.register.captcha}</span>
          <div className="flex items-center gap-2">
            <span
              className="num min-h-[44px] flex items-center
                rounded-[var(--r-sm)] border border-line bg-cloud px-3
                font-bold"
            >
              {captcha?.question ?? "…"}
            </span>
            <input
              type="text"
              inputMode="numeric"
              value={captchaAnswer}
              onChange={(e) =>
                setCaptchaAnswer(e.target.value.replace(/\D/g, ""))
              }
              required
              placeholder={dict.register.captchaAnswer}
              className={`${inputCls} min-w-0 flex-1`}
            />
            <button
              type="button"
              onClick={refreshCaptcha}
              aria-label={dict.register.captchaRefresh}
              className="min-h-[44px] shrink-0 rounded-full border border-line
                px-3 text-sm text-sub active:scale-[0.97]"
            >
              ↻
            </button>
          </div>
        </label>
        )}
        {msg && (
          <p role="alert" className="text-sm text-danger">
            {msg}
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
          {busy ? dict.register.busy : dict.register.submit}
        </button>
        <RegisterFields
          fields={regFields}
          values={fieldVals}
          onChange={setFieldVals}
        />
      </form>
      <p className="text-sm text-sub">
        {dict.register.hasAccount}{" "}
        <Link href="/login" className="font-bold text-sky">
          {dict.register.toLogin}
        </Link>
      </p>
    </div>
  );
}
