"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** Passkey（WebAuthn）绑定卡（0227，P2）：
 *  usercp 安全设置内嵌——列出已绑凭证、浏览器注册新凭证（ES256）、解绑。
 *  登录侧入口在 /login 页（passkey-login begin/finish）。 */

interface PasskeyRow {
  label: string;
  cred_id: string;
  created_at: string;
  last_used_at: string | null;
}

const B64URL = (buf: ArrayBuffer) =>
  btoa(String.fromCharCode(...new Uint8Array(buf)))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");

export function PasskeysCard() {
  const { dict, locale } = useI18n();
  const [rows, setRows] = useState<PasskeyRow[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // P-107：begin 现在要复核凭证（密码，开了 2FA 还要当前码）才发 challenge
  const [step, setStep] = useState(false);
  const [pw, setPw] = useState("");
  const [code, setCode] = useState("");
  const [twofa, setTwofa] = useState(false);

  const load = useCallback(async () => {
    try {
      const r = await api.get<{ items: PasskeyRow[] }>(
        "/api/v1/me/passkeys",
      );
      setRows(r.items ?? []);
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    void load();
    // 启用态用 /me/overview 的 totp_enabled 探测（与 twofa-setup 同一口径），
    // 只为决定「要不要收验证码」——服务端自己一定再核一遍，这里不放宽。
    void api
      .get<{ totp_enabled?: boolean }>("/api/v1/me/overview")
      .then((o) => setTwofa(Boolean(o?.totp_enabled)))
      .catch(() => setTwofa(false));
  }, [load]);

  async function register() {
    setBusy(true);
    setMsg(null);
    try {
      // P-112：设备名要在 begin **之前**问——begin 把 label 与 challenge 一起
      // 存下，finish 再从库里取回；旧流程先 begin 后 prompt，用户填的名字
      // 只进了 finish 的 body（该结构体根本没有 label 字段），于是所有凭证
      // 一律叫「passkey」。
      const label =
        window.prompt(dict.usercp.security.webauthnNamePrompt, "") ||
        dict.usercp.security.webauthnNameFallback;
      const begin = await api.post<{
        challenge: string;
        rp_id: string;
        user_id: number;
      }>("/api/v1/auth/passkeys/begin", {
        label,
        password: pw,
        ...(twofa ? { totp: Number(code) } : {}),
      });
      const cred = await navigator.credentials.create({
        publicKey: {
          challenge: new TextEncoder().encode(begin.challenge),
          rp: { id: begin.rp_id, name: document.title || "FluxTorrent" },
          user: {
            id: new Uint8Array([begin.user_id & 0xff]).buffer as ArrayBuffer,
            name: `uid-${begin.user_id}`,
            displayName: label,
          },
          pubKeyCredParams: [{ type: "public-key", alg: -7 }],
          authenticatorSelection: { userVerification: "preferred" },
          timeout: 60_000,
        },
      });
      if (!cred) throw new Error(dict.usercp.security.webauthnCancelErr);
      const c = cred as PublicKeyCredential;
      const r = c.response as AuthenticatorAttestationResponse & {
        getAuthenticatorData?: () => ArrayBuffer;
      };
      // 公钥取 getPublicKey()（Chrome 103+/Safari 16.4+），它返回的是
      // **DER SubjectPublicKeyInfo**（WebAuthn L2 规定，MDN 同）；服务端在
      // 入口把它归一成库里存的 SEC1 未压缩点（P-82：此前服务端只认 SEC1，
      // 浏览器交 SPKI 一律「公钥格式错误」，passkey 对浏览器从未绑成功过）。
      // 注意：不是 COSE——COSE_Key 在 attestationObject/authData 里，
      // 服务端明确拒收（闸门 AV S1c 钉住这个口径）。
      const pk = (
        r as AuthenticatorAttestationResponse & {
          getPublicKey?: () => ArrayBuffer | null;
        }
      ).getPublicKey?.();
      if (!pk) throw new Error(dict.usercp.security.webauthnNoPkErr);
      await api.post("/api/v1/auth/passkeys/finish", {
        cred_id: B64URL(c.rawId),
        public_key: B64URL(pk),
        client_data_json: B64URL(r.clientDataJSON),
        auth_data: B64URL(r.getAuthenticatorData?.() ?? new ArrayBuffer(0)),
        sign_count: 0,
        label,
      });
      setMsg(dict.usercp.security.webauthnBindOk);
      setStep(false);
      setPw("");
      setCode("");
      await load();
    } catch (e) {
      // P-115：WebAuthn 抛的是 DOMException，`name` 才是可分类的信息，
      // message 是写给开发者看的英文（含 w3.org 链接）。面板原先把它原文
      // 甩在界面上，用户既看不懂也不知道下一步该做什么。
      const nm =
        e instanceof DOMException || e instanceof Error ? e.name : "";
      const sec = dict.usercp.security;
      const hint =
        nm === "NotAllowedError"
          ? sec.webauthnErrNotAllowed
          : nm === "SecurityError"
            ? sec.webauthnErrSecurity
            : nm === "TimeoutError"
              ? sec.webauthnErrTimeout
              : null;
      setMsg(hint ?? (e instanceof ApiError ? e.message : String(e)));
    } finally {
      setBusy(false);
    }
  }

  async function remove(credId: string) {
    setBusy(true);
    try {
      await api.post("/api/v1/auth/passkeys/remove", { cred_id: credId });
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel flex flex-col gap-2 p-4">
      <div className="flex items-center justify-between">
        <span className="text-sm font-semibold">
          {dict.usercp.security.webauthnTitle}
        </span>
        <button
          type="button"
          disabled={busy}
          onClick={() => setStep(true)}
          className="min-h-[32px] rounded-full bg-sky-deep px-3 text-xs
            font-bold text-white disabled:opacity-50"
        >
          {dict.usercp.security.webauthnBind}
        </button>
      </div>
      {step && (
        <div className="flex flex-col gap-2 rounded border border-line p-3">
          <p className="text-xs text-sub">
            {dict.usercp.security.webauthnStepUpHint}
          </p>
          <input
            className="min-h-[32px] rounded border border-line bg-cloud px-3
              text-xs"
            type="password"
            autoComplete="current-password"
            placeholder={dict.usercp.security.webauthnPwPh}
            value={pw}
            onChange={(e) => setPw(e.target.value)}
          />
          {twofa && (
            <input
              className="min-h-[32px] rounded border border-line bg-cloud
                px-3 text-xs"
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={6}
              placeholder={dict.usercp.security.webauthnCodePh}
              value={code}
              onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))}
            />
          )}
          <button
            type="button"
            disabled={busy || !pw || (twofa && code.length < 6)}
            onClick={() => void register()}
            className="self-start rounded-full bg-sky-deep px-3 py-1 text-xs
              font-bold text-white disabled:opacity-50"
          >
            {dict.usercp.security.webauthnStepUpBtn}
          </button>
        </div>
      )}
      {(rows ?? []).length === 0 && (
        <p className="text-xs text-sub">
          {rows === null ? "…" : dict.usercp.security.webauthnEmpty}
        </p>
      )}
      {(rows ?? []).map((r) => (
        <div
          key={r.cred_id}
          className="flex items-center justify-between gap-2
            rounded border border-line px-3 py-2 text-xs"
        >
          <span className="min-w-0 flex-1 truncate">
            {r.label || dict.usercp.security.webauthnNameFallback}
            <span className="ml-2 text-fainter">
              {r.last_used_at
                ? fmt(dict.usercp.security.webauthnLastUsed, {
                    date: new Date(r.last_used_at).toLocaleDateString(
                      dateLocale(locale),
                    ),
                  })
                : fmt(dict.usercp.security.webauthnBoundAt, {
                    date: new Date(r.created_at).toLocaleDateString(
                      dateLocale(locale),
                    ),
                  })}
            </span>
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={() => void remove(r.cred_id)}
            className="shrink-0 text-danger disabled:opacity-50"
          >
            {dict.usercp.security.webauthnUnbind}
          </button>
        </div>
      ))}
      {msg && <p className="text-xs text-sub">{msg}</p>}
    </section>
  );
}
