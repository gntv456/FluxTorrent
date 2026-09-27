"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

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
  const { dict } = useI18n();
  void dict;
  const [rows, setRows] = useState<PasskeyRow[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

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
  }, [load]);

  async function register() {
    setBusy(true);
    setMsg(null);
    try {
      const begin = await api.post<{
        challenge: string;
        rp_id: string;
        user_id: number;
      }>("/api/v1/auth/passkeys/begin", { label: "" });
      const label =
        window.prompt("给这枚凭证起个名字（如：笔记本）", "") || "passkey";
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
      if (!cred) throw new Error("浏览器取消了注册");
      const c = cred as PublicKeyCredential;
      const r = c.response as AuthenticatorAttestationResponse & {
        getAuthenticatorData?: () => ArrayBuffer;
      };
      // 公钥从 attestationObject 里取太深——服务端兼容「裸 COSE 公钥」提交：
      // 现代浏览器可用 getPublicKey()（Chrome 103+/Safari 16.4+）
      const pk = (
        r as AuthenticatorAttestationResponse & {
          getPublicKey?: () => ArrayBuffer | null;
        }
      ).getPublicKey?.();
      if (!pk) throw new Error("浏览器不支持 getPublicKey()，请升级");
      await api.post("/api/v1/auth/passkeys/finish", {
        cred_id: B64URL(c.rawId),
        public_key: B64URL(pk),
        client_data_json: B64URL(r.clientDataJSON),
        auth_data: B64URL(r.getAuthenticatorData?.() ?? new ArrayBuffer(0)),
        sign_count: 0,
        label,
      });
      setMsg("绑定成功");
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
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
          Passkey（指纹 / 安全钥匙）
        </span>
        <button
          type="button"
          disabled={busy}
          onClick={() => void register()}
          className="min-h-[32px] rounded-full bg-sky-deep px-3 text-xs
            font-bold text-white disabled:opacity-50"
        >
          绑定新凭证
        </button>
      </div>
      {(rows ?? []).length === 0 && (
        <p className="text-xs text-sub">
          {rows === null ? "…" : "尚未绑定 Passkey。绑定后可在登录页免密登录。"}
        </p>
      )}
      {(rows ?? []).map((r) => (
        <div
          key={r.cred_id}
          className="flex items-center justify-between gap-2
            rounded border border-line px-3 py-2 text-xs"
        >
          <span className="min-w-0 flex-1 truncate">
            {r.label || "passkey"}
            <span className="ml-2 text-fainter">
              {r.last_used_at
                ? `最近使用 ${new Date(r.last_used_at).toLocaleDateString()}`
                : `绑定于 ${new Date(r.created_at).toLocaleDateString()}`}
            </span>
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={() => void remove(r.cred_id)}
            className="shrink-0 text-danger disabled:opacity-50"
          >
            解绑
          </button>
        </div>
      ))}
      {msg && <p className="text-xs text-sub">{msg}</p>}
    </section>
  );
}
