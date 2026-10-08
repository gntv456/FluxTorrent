"use client";

/**
 * C7-#4 邮箱激活页：邮件链接落点（/auth/email/verify?token=...）。
 * 一次性 token：激活成功提示去登录；过期/已用给「重发」出口。
 */
import { Suspense, useCallback, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

function VerifyInner() {
  const { dict } = useI18n();
  const sp = useSearchParams();
  const token = sp.get("token") ?? "";
  const [state, setState] = useState<"idle" | "ok" | "already" | "err">(
    "idle",
  );
  const [msg, setMsg] = useState("");
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
    setMsg("");
    try {
      await api.post("/api/v1/auth/email/resend", { email });
      setResent(true);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    }
  }, [email]);

  return (
    <div className="mx-auto flex max-w-md flex-col gap-4">
      <div className="card p-6">
        <h1 className="font-display text-xl">{dict.emailVerify.title}</h1>
        {state === "idle" && (
          <div className="mt-4 flex flex-col gap-3">
            <p className="text-sm text-muted">
              {fmt(dict.emailVerify.intro, { hours: 48 })}
            </p>
            <button
              className="btn btn-primary"
              disabled={!token}
              onClick={() => void verify()}
            >
              {dict.emailVerify.verifyBtn}
            </button>
          </div>
        )}
        {state === "ok" && (
          <p className="mt-4 text-sm">
            {dict.emailVerify.okPrefix}
            <a className="text-sky" href="/login">
              {dict.emailVerify.okLink}
            </a>
            {dict.emailVerify.okSuffix}
          </p>
        )}
        {state === "already" && (
          <p className="mt-4 text-sm">
            {dict.emailVerify.alreadyPrefix}
            <a className="text-sky" href="/login">
              {dict.emailVerify.alreadyLink}
            </a>
            {dict.emailVerify.alreadySuffix}
          </p>
        )}
        {state === "err" && (
          <div className="mt-4 flex flex-col gap-3">
            <p className="text-sm text-destructive">{msg}</p>
            <p className="text-xs text-muted">{dict.emailVerify.expiredHint}</p>
            <div className="flex gap-2">
              <input
                className="min-h-[38px] flex-1 rounded border border-line
                  bg-cloud px-3 text-sm"
                placeholder={dict.emailVerify.emailPh}
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                inputMode="email"
              />
              <button
                className="btn text-sm"
                disabled={!email.includes("@")}
                onClick={() => void resend()}
              >
                {dict.emailVerify.resendBtn}
              </button>
            </div>
            {resent && (
              <p className="text-xs text-muted">{dict.emailVerify.resentNote}</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

export default function EmailVerifyPage() {
  return (
    <Suspense fallback={<div className="p-8 text-center text-muted">…</div>}>
      <VerifyInner />
    </Suspense>
  );
}
