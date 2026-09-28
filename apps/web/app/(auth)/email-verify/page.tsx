"use client";

/**
 * C7-#4 邮箱激活页：邮件链接落点（/auth/email/verify?token=...）。
 * 一次性 token：激活成功提示去登录；过期/已用给「重发」出口。
 */
import { Suspense, useCallback, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";

function VerifyInner() {
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
        <h1 className="font-display text-xl">邮箱激活</h1>
        {state === "idle" && (
          <div className="mt-4 flex flex-col gap-3">
            <p className="text-sm text-muted">
              点击下方按钮完成激活（链接 48 小时内有效，一次性使用）。
            </p>
            <button
              className="btn btn-primary"
              disabled={!token}
              onClick={() => void verify()}
            >
              激活我的账号
            </button>
          </div>
        )}
        {state === "ok" && (
          <p className="mt-4 text-sm">
            ✅ 激活成功！现在可以
            <a className="text-sky" href="/login">去登录</a> 了。
          </p>
        )}
        {state === "already" && (
          <p className="mt-4 text-sm">
            该链接已被使用过——账号若已激活，直接
            <a className="text-sky" href="/login"> 登录</a>即可。
          </p>
        )}
        {state === "err" && (
          <div className="mt-4 flex flex-col gap-3">
            <p className="text-sm text-destructive">{msg}</p>
            <p className="text-xs text-muted">
              链接过期？输入注册邮箱重新发送激活邮件。
            </p>
            <div className="flex gap-2">
              <input
                className="min-h-[38px] flex-1 rounded border border-line
                  bg-cloud px-3 text-sm"
                placeholder="注册邮箱"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                inputMode="email"
              />
              <button
                className="btn text-sm"
                disabled={!email.includes("@")}
                onClick={() => void resend()}
              >
                重新发送
              </button>
            </div>
            {resent && (
              <p className="text-xs text-muted">
                若该邮箱有未激活账号，激活邮件已发出（1 分钟内勿重复发送）。
              </p>
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
