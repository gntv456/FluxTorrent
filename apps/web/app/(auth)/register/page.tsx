"use client";

import { INPUT_LG } from "@/lib/ui-classes";

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
import {
  FormField,
  FieldErrors,
  fieldErrorsFrom,
} from "@/components/ui/form-field";
import { SubmitButton } from "@/components/ui/submit-button";

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

/** W-18 试点：错误码 → 字段级渲染的映射。后端注册接口多为单错误码
 *  （2005 邀请码 / 2007 用户名占用 / 2011 验证码…），按码归到对应
 *  字段让 FormField 内联红字；1002 多字段信封走 fieldErrorsFrom。 */
const CODE_TO_FIELD: Record<number, string> = {
  2005: "invite_code", // 邀请码无效或已过期
  2006: "invite_code", // 邀请码已被使用
  2007: "username", // 用户名已被占用
  1002: "", // 多字段信封，fieldErrorsFrom 解析
};

/** 注册页（M01 邀请制）：邀请码 + 算术验证码 + 账号密码，对接 POST /auth/register。
 *  W-18 表单协议试点：FormField 四槽位 + FieldErrors 汇总 + SubmitButton
 *  生命周期（busy 防双击）；错误按码映射到字段内联渲染。 */
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
  // W-18：字段级错误 map（字段名 → 消息）；表单级错误降级为 form 键
  const [fieldErrs, setFieldErrs] = useState<Record<string, string[]>>({});
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

  /** W-18 错误路径：后端错 → 字段级 map。优先 1002 多字段信封
   *  （fieldErrorsFrom），单错误码按 CODE_TO_FIELD 归字段；归不了的
   *  落 form 键由 FieldErrors 顶部汇总渲染。 */
  function mapError(err: unknown): Record<string, string[]> {
    const fromEnvelope = fieldErrorsFrom(err);
    if (fromEnvelope) {
      return Object.fromEntries(
        Object.entries(fromEnvelope).map(([f, v]) => [
          f,
          Array.isArray(v) ? v : [v],
        ]),
      );
    }
    if (err instanceof ApiError) {
      const msg = dict.errors[err.code] ?? err.message;
      const field = CODE_TO_FIELD[err.code];
      return field ? { [field]: [msg] } : { form: [msg] };
    }
    return { form: [dict.common.networkError] };
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setFieldErrs({});
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
      await api.post<{ token: string }>("/api/v1/auth/login", {
        username: username.trim(),
        password,
      });
      setSessionCookie(true);
      router.push("/torrents");
    } catch (err) {
      setFieldErrs(mapError(err));
      refreshCaptcha(); // 验证码一次性，失败即换新题
    } finally {
      setBusy(false);
    }
  }

  const first = (f: string) => fieldErrs[f]?.[0];

  return (
    <div className="mx-auto flex max-w-sm flex-col items-center gap-6 py-8">
      {/* M5.1：品牌头由 (auth) 壳统一渲染，页内只留标题 */}
      <h1 className="font-display text-2xl">{dict.register.title}</h1>
      <p className="-mt-4 text-sm text-sub">{dict.register.subtitle}</p>
      <form onSubmit={submit} className="flex w-full flex-col gap-3">
        {/* W-18：表单级错误（含映射不出的码）走 FieldErrors 汇总 */}
        <FieldErrors fields={fieldErrs} />
        <FormField
          name="invite_code"
          label={dict.register.inviteCode}
          required={!openMode}
          error={first("invite_code")}
          hint={openMode ? dict.register.inviteOptional : undefined}
        >
          <input
            value={inviteCode}
            onChange={(e) => setInviteCode(e.target.value)}
            required={!openMode}
            maxLength={32}
            className={INPUT_LG}
          />
        </FormField>
        <FormField
          name="username"
          label={dict.register.username}
          required
          error={first("username")}
        >
          <input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            required
            minLength={3}
            maxLength={24}
            className={INPUT_LG}
          />
        </FormField>
        <FormField
          name="email"
          label={dict.register.email}
          required
          error={first("email")}
          hint={inviteCode ? dict.register.emailMustMatch : undefined}
        >
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="email"
            required
            className={INPUT_LG}
          />
        </FormField>
        <FormField
          name="password"
          label={dict.register.password}
          required
          error={first("password")}
          hint={dict.register.passwordHint}
        >
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="new-password"
            required
            minLength={8}
            className={INPUT_LG}
          />
        </FormField>
        {/* 验证码（0227）：none=自研算术题；第三方=对应组件（token 由回调写入） */}
        {driver !== "none" && siteKey ? (
          <div id={DRIVER_WIDGET_ID[driver]} className="min-h-[64px]" />
        ) : null}
        {/* 算术验证码（后端 /auth/captcha 出题，一次性） */}
        {driver === "none" && (
          <FormField
            name="captcha"
            label={dict.register.captcha}
            required
            error={first("captcha")}
          >
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
                className={`${INPUT_LG} min-w-0 flex-1`}
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
          </FormField>
        )}
        <SubmitButton
          busy={busy}
          label={dict.register.submit}
          busyLabel={dict.register.busy}
        />
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
