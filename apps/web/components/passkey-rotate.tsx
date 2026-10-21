"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";

const INPUT_CLS =
  "w-40 rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2 py-1 text-sm";
const BTN_CLS =
  "min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold " +
  "text-white disabled:opacity-50";

/** 重置下载密钥（P-409）：轮换一次就发新钥、废旧钥、撤下载键与 RSS。
 *  会话被偷的人不能靠一次请求把原主所有下载器打成 401，也不该白拿一份
 *  新的 tracker/RSS 身份——与改密同一把尺，先复核本人；开了 2FA 还要
 *  当前动态码（step_up 的口径）。 */
export function PasskeyRotate() {
  const { dict } = useI18n();
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const ask = useConfirm();
  // 2FA 的标签由 security2fa 字典提供；缺字典时用同一句中文兜底
  // （与 twofa-setup.tsx 同款，不新增 i18n 键）。
  const codeLabel = dict.security2fa?.codeLabel ?? "验证器 6 位动态码";

  async function rotateNow() {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ passkey: string }>(
        "/api/v1/me/passkey/rotate",
        {
          password,
          code: code.trim() ? parseInt(code, 10) : undefined,
        },
      );
      setResult(r.passkey);
      setMsg(dict.my.passkeyRotated);
      setPassword("");
      setCode("");
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? apiErrorMessage(dict, e)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          placeholder={dict.usercp.security.oldPassword}
          aria-label={dict.usercp.security.oldPassword}
          className={INPUT_CLS}
        />
        <input
          inputMode="numeric"
          value={code}
          onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))}
          autoComplete="one-time-code"
          placeholder={codeLabel}
          aria-label={codeLabel}
          maxLength={6}
          className={INPUT_CLS}
        />
        <button
          type="button"
          disabled={busy || !password}
          onClick={() =>
            ask.open({
              title: dict.my.passkeyRotate,
              message: dict.my.passkeyConfirm,
              tone: "danger",
              busy,
              run: rotateNow,
            })
          }
          className={BTN_CLS}
        >
          {busy ? dict.my.passkeyRotating : dict.my.passkeyRotate}
        </button>
      </div>
      {result && (
        <>
          <br />
          <code className="num break-all text-xs">{result}</code>
        </>
      )}
      {msg && <p className="text-xs text-sub">{msg}</p>}
      <AskDialog {...ask} />
    </>
  );
}
