"use client";

import { INPUT_LG } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api, setSessionCookie, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { AuthCanvas } from "@/components/auth-canvas";
import { mountCaptchaWidget } from "@/lib/captcha-widget";
import {
  RegisterFields,
  type RegField,
} from "@/components/register-fields";
import {
  RegisterCaptcha,
  RegisterPow,
  RegisterQuiz,
} from "@/components/register-gates";
import {
  FormField,
  FieldErrors,
  fieldErrorsFrom,
} from "@/components/ui/form-field";
import { SubmitButton } from "@/components/ui/submit-button";
import { RegisterBasicFields } from "./register-basic-fields";

interface Captcha {
  captcha_id: string;
  question: string;
}

/// register-mode 下发的公开配置（0227）：验证码驱动 + 申请通道 + 答题闸
interface ModeInfo {
  mode?: string;
  captcha_provider?: string;
  captcha_site_key?: string;
  application_signup?: string;
  reg_quiz_enabled?: boolean;
  reg_quiz_pass_required?: boolean;
}

/** W-18：错误码→字段映射（2005 邀请码 / 2007 用户名；1002 走信封）。 */
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
  const [regFields, setRegFields] = useState<RegField[]>([]);
  const [fieldVals, setFieldVals] = useState<Record<string, unknown>>({});
  const [captcha, setCaptcha] = useState<Captcha | null>(null);
  // W-18：字段级错误 map（字段名 → 消息）；表单级错误降级为 form 键
  const [fieldErrs, setFieldErrs] = useState<Record<string, string[]>>({});
  const [busy, setBusy] = useState(false);
  // 注册模式（0204）：open 不强制邀请码；invite_only 需要。
  // 0236 回补的 email_verify = 开放注册 + 邮件激活：注册口同样不需要
  // 邀请码（后端只在 invite_only 分支校验），旧写法只认 open ⇒ 选择该
  // 模式后浏览器按 required 挡住提交，开放注册整条走不通（P-567）。
  const [openMode, setOpenMode] = useState(false);
  // email_verify：注册成功后不硬登（guard_login 必然拦下），改落激活页
  const [verifyMode, setVerifyMode] = useState(false);
  // 0227：验证码驱动（none=自研算术题，其他=第三方组件 token）
  const [driver, setDriver] = useState("none");
  const [siteKey, setSiteKey] = useState("");
  const [captchaToken, setCaptchaToken] = useState("");
  // 0372 M5：答题闸（reg_quiz_enabled）+ PoW 凭据（captcha_provider=pow）
  const [quizOn, setQuizOn] = useState(false);
  const [quizHard, setQuizHard] = useState(true);
  const [quizTicket, setQuizTicket] = useState("");
  const [quizAnswer, setQuizAnswer] = useState(-1);
  const [pow, setPow] = useState({ token: "", solution: "" });
  /** 一次性凭据失效后重抽（验证码/答题票据/PoW 解都是 GETDEL 语义） */
  const [gateNonce, setGateNonce] = useState(0);

  const onQuizPick = useCallback((ticket: string, answer: number) => {
    setQuizTicket(ticket);
    setQuizAnswer(answer);
  }, []);
  const onPowSolved = useCallback((token: string, solution: string) => {
    setPow({ token, solution });
  }, []);

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
        setOpenMode(r.mode === "open" || r.mode === "email_verify");
        setVerifyMode(r.mode === "email_verify");
        const prov = r.captcha_provider ?? "none";
        setDriver(prov);
        setSiteKey(r.captcha_site_key ?? "");
        setQuizOn(r.reg_quiz_enabled === true);
        setQuizHard(r.reg_quiz_pass_required !== false);
      })
      .catch(() => setOpenMode(false));
  }, []);

  /** 验证码组件挂载（P-504）：token 一次性，失败后靠 gateNonce 换 key 重挂；
   *  ref 去重防 StrictMode 开发态二次执行把同一容器渲染两遍。 */
  const mounted = useRef("");
  useEffect(() => {
    if (driver === "none" || !siteKey) return;
    const k = `${driver}|${siteKey}|${gateNonce}`;
    if (mounted.current === k) return;
    mounted.current = k;
    mountCaptchaWidget(driver, siteKey, setCaptchaToken);
  }, [driver, siteKey, gateNonce]);

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
      // P-568：1002 的 message 是「参数校验失败: 具体判据」，取正文，
      // 别用字典标签把「邮箱被禁用」这类可操作的正确原因盖掉。
      const msg = apiErrorMessage(dict, err);
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
        pow_token: pow.token,
        pow_solution: pow.solution,
        quiz_ticket: quizTicket,
        quiz_answer: quizAnswer,
        fields: fieldVals,
      });
      // P-567：email_verify 模式下账号刚建、email_verified_at 为空，
      // 直接登会被 guard_login 拦下 ⇒ 落激活页（email 预填，方便重发）。
      if (verifyMode) {
        router.push(
          `/email-verify?email=${encodeURIComponent(email.trim())}`,
        );
        return;
      }
      // 注册成功直接走登录（后端注册不返回 token）
      await api.post<{ token: string }>("/api/v1/auth/login", {
        username: username.trim(),
        password,
      });
      setSessionCookie(true);
      router.push("/torrents");
    } catch (err) {
      setFieldErrs(mapError(err));
      // 凭据一次性：验证码 token / 算术答案 / PoW 解与服务端 GETDEL 同寿命
      setCaptchaToken("");
      setCaptchaAnswer("");
      refreshCaptcha(); setGateNonce((n) => n + 1);
    } finally {
      setBusy(false);
    }
  }

  const first = (f: string) => fieldErrs[f]?.[0];

  return (
    <AuthCanvas
      title={dict.register.title}
      desc={dict.register.subtitle}
      heroTag={dict.register.heroTag}
      heroDesc={dict.register.heroDesc}
      badge={
        openMode ? undefined : (
          <>
            {dict.register.inviteBadge} ·{" "}
            <Link href="/apply" className="underline">
              {dict.register.applyForCode}
            </Link>
          </>
        )
      }
      foot={dict.register.termsNote}
    >
      <form onSubmit={submit} className="flex w-full flex-col gap-3">
        {/* W-18：表单级错误（含映射不出的码）走 FieldErrors 汇总 */}
        <FieldErrors fields={fieldErrs} />
        <RegisterBasicFields
          values={{ inviteCode, username, email, password }}
          onChange={(patch) => {
            if (patch.inviteCode !== undefined) setInviteCode(patch.inviteCode);
            if (patch.username !== undefined) setUsername(patch.username);
            if (patch.email !== undefined) setEmail(patch.email);
            if (patch.password !== undefined) setPassword(patch.password);
          }}
          first={first}
          openMode={openMode}
        />
        {/* 验证码（0227）：none=自研算术题；pow=自托管 PoW；其他=第三方组件 */}
        {driver === "pow" ? (
          <RegisterPow key={gateNonce} onSolved={onPowSolved}
            error={first("pow")} />
        ) : (
          <RegisterCaptcha
            key={gateNonce}
            driver={driver}
            siteKey={siteKey}
            captcha={captcha}
            answer={captchaAnswer}
            onAnswer={setCaptchaAnswer}
            onRefresh={refreshCaptcha}
            error={first("captcha")}
          />
        )}
        {/* 站规答题（0372 M5）：reg_quiz_enabled=yes 才出现 */}
        {quizOn && (
          <RegisterQuiz
            hard={quizHard} key={gateNonce}
            onChange={onQuizPick}
            error={first("quiz")}
          />
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
      <div className="auth-card__links">
        <span>
          {dict.register.hasAccount}{" "}
          <Link href="/login">{dict.register.toLogin}</Link>
        </span>
      </div>
    </AuthCanvas>
  );
}
