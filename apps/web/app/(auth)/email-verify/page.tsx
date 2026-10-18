"use client";

/**
 * C7-#4 邮箱激活页：邮件链接落点（/auth/email/verify?token=...）。
 * 一次性 token：激活成功提示去登录；过期/已用给「重发」出口。
 *
 * 2026-10-18 对齐样图 § 账户与认证：改用认证画布。卡内按样图排：
 * 状态徽章 + 时效提示条 → 验证按钮 →「或」分隔 → 重发激活邮件
 * （输入框 + 重发）→ 返回登录。占位/按钮换回项目按钮原语
 * （旧版用的 .btn-primary 在样式表里没有定义，主按钮渲染成次按钮）。
 */
import { Suspense, useCallback, useState } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";
import { AuthCanvas } from "@/components/auth-canvas";

function VerifyInner() {
  const { dict } = useI18n();
  const sp = useSearchParams();
  const token = sp.get("token") ?? "";
  const [state, setState] = useState<"idle" | "ok" | "already" | "err">(
    "idle",
  );
  const [msg, setMsg] = useState("");
  const [rmsg, setRmsg] = useState("");
  const [email, setEmail] = useState("");
  const [resent, setResent] = useState(false);

  const verify = useCallback(async () => {
    setMsg("");
    try {
      const r = await api.get<{ verified: boolean; already: boolean }>(
        `/api/v1/auth/email/verify?token=${encodeURIComponent(token)}`,
      );
      setState(r.already ? "already" : "ok");
    } catch (e) {
      setState("err");
      setMsg(e instanceof ApiError ? e.message : String(e));
    }
  }, [token]);

  const resend = useCallback(async () => {
    setResent(false);
    setRmsg("");
    try {
      await api.post("/api/v1/auth/email/resend", { email });
      setResent(true);
    } catch (e) {
      setRmsg(e instanceof ApiError ? e.message : String(e));
    }
  }, [email]);

  const resendForm = (
    <div className="flex flex-col gap-2">
      <label className="flex flex-col gap-1">
        <span className="text-xs text-sub">
          {dict.emailVerify.resendLabel}
        </span>
        <div className="flex gap-2">
          <input
            className={`${INPUT_LG} flex-1`}
            placeholder={dict.emailVerify.emailPh}
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            inputMode="email"
          />
          <button
            type="button"
            className="min-h-[44px] shrink-0 rounded-full border
              border-line bg-[var(--surface-card)] px-4 text-xs font-bold
              text-ink disabled:opacity-40"
            disabled={!email.includes("@")}
            onClick={() => void resend()}
          >
            {dict.emailVerify.resendBtn}
          </button>
        </div>
      </label>
      {/* P-110：重发的失败必须有落点。旧版把错误写进 msg，而 msg 只在
          「验证失败」那一支渲染——空手进来（没带 token）的用户点重发，
          限流/未配邮件服务时界面一动不动，像按钮坏了。 */}
      {rmsg && <p className="text-sm text-danger">{rmsg}</p>}
      {resent && (
        <p className="text-xs text-sub">{dict.emailVerify.resentNote}</p>
      )}
    </div>
  );

  return (
    <AuthCanvas
      title={dict.emailVerify.title}
      heroTag={dict.emailVerify.heroTag}
      heroDesc={dict.emailVerify.heroDesc}
      badge={dict.emailVerify.badge}
      notice={
        token
          ? fmt(dict.emailVerify.intro, { hours: 48 })
          : dict.emailVerify.noTokenHint
      }
    >
      <div className="flex flex-col gap-3">
        {state === "idle" && token && (
          <button
            type="button"
            className={BTN_LG_SKY}
            onClick={() => void verify()}
          >
            {dict.emailVerify.verifyBtn}
          </button>
        )}
        {state === "idle" && !token && resendForm}
        {state === "ok" && (
          <p className="text-sm">
            {dict.emailVerify.okPrefix}
            <Link className="font-bold text-sky" href="/login">
              {dict.emailVerify.okLink}
            </Link>
            {dict.emailVerify.okSuffix}
          </p>
        )}
        {state === "already" && (
          <p className="text-sm">
            {dict.emailVerify.alreadyPrefix}
            <Link className="font-bold text-sky" href="/login">
              {dict.emailVerify.alreadyLink}
            </Link>
            {dict.emailVerify.alreadySuffix}
          </p>
        )}
        {state === "err" && (
          <div className="flex flex-col gap-3">
            <p className="text-sm text-danger">{msg}</p>
            <p className="text-xs text-sub">{dict.emailVerify.expiredHint}</p>
            {resendForm}
          </div>
        )}
        {/* 「或」分隔（样图：一键验证 / 重发激活并排两条路） */}
        {state === "idle" && token && (
          <>
            <p className="auth-card__or">{dict.emailVerify.or}</p>
            {resendForm}
          </>
        )}
      </div>
      <div className="auth-card__links">
        <Link href="/login">{dict.forgot.backToLogin}</Link>
      </div>
    </AuthCanvas>
  );
}

export default function EmailVerifyPage() {
  return (
    <Suspense
      fallback={
        <p className="py-12 text-center text-sm text-sub">…</p>
      }
    >
      <VerifyInner />
    </Suspense>
  );
}
