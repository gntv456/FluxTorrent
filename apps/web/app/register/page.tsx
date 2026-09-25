"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";
import { Icon } from "@/components/icons";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api, setSessionCookie, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface Captcha {
  captcha_id: string;
  question: string;
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

  async function refreshCaptcha() {
    try {
      setCaptcha(await api.get<Captcha>("/api/v1/auth/captcha"));
    } catch {
      setCaptcha(null);
    }
  }

  useEffect(() => {
    refreshCaptcha();
    fetch("/api/v1/register-fields")
      .then((r) => r.json())
      .then((b: { data?: typeof regFields }) =>
        setRegFields(b?.data ?? []),
      )
      .catch(() => setRegFields([]));
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
        captcha_id: captcha?.captcha_id ?? "",
        captcha_answer: captchaAnswer ? Number(captchaAnswer) : 0,
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
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-12">
      <Icon name="seed" size={72} className="text-[var(--sky)]" />
      <h1 className="font-display text-3xl">{dict.register.title}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.register.subtitle}</p>
      <form onSubmit={submit} className="flex w-full flex-col gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.register.inviteCode}</span>
          <input
            value={inviteCode}
            onChange={(e) => setInviteCode(e.target.value)}
            required
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
        {/* 算术验证码（后端 /auth/captcha 出题，一次性） */}
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.register.captcha}</span>
          <div className="flex items-center gap-2">
            <span className="num min-h-[44px] flex items-center rounded-[var(--r-sm)] border border-line bg-cloud px-3 font-bold">
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
              className="min-h-[44px] shrink-0 rounded-full border border-line px-3 text-sm text-sub active:scale-[0.97]"
            >
              ↻
            </button>
          </div>
        </label>
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
        {regFields.map((f) => (
          <label key={f.key} className="flex flex-col gap-1">
            <span className="text-sm text-sub">
              {f.label}
              {f.required ? " *" : ""}
            </span>
            {f.type === "select" ? (
              <select
                className={inputCls}
                required={f.required}
                value={String(fieldVals[f.key] ?? "")}
                onChange={(e) =>
                  setFieldVals((p) => ({
                    ...p,
                    [f.key]: e.target.value || null,
                  }))
                }
              >
                <option value="">—</option>
                {f.options.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label ?? o.value}
                  </option>
                ))}
              </select>
            ) : f.type === "bool" ? (
              <input
                type="checkbox"
                checked={fieldVals[f.key] === true}
                onChange={(e) =>
                  setFieldVals((p) => ({
                    ...p,
                    [f.key]: e.target.checked,
                  }))
                }
              />
            ) : (
              <input
                type={
                  f.type === "number"
                    ? "number"
                    : f.type === "date"
                      ? "date"
                      : "text"
                }
                className={inputCls}
                required={f.required}
                maxLength={f.type === "text" ? 500 : undefined}
                value={String(fieldVals[f.key] ?? "")}
                onChange={(e) =>
                  setFieldVals((p) => ({
                    ...p,
                    [f.key]:
                      f.type === "number"
                        ? e.target.value === ""
                          ? null
                          : Number(e.target.value)
                        : e.target.value || null,
                  }))
                }
              />
            )}
          </label>
        ))}
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
