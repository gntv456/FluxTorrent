/** Passkey 登录 hook（0227）：begin→浏览器断言→finish→session 跳转。
 *  从 login-form.tsx 拆出（300 行门禁）。错误统一抛给表单层展示。 */

import { api, setSessionCookie, ApiError } from "@/lib/api-client";
import { useRouter } from "next/navigation";
import { useCallback } from "react";

const B64URL = (buf: ArrayBuffer) =>
  btoa(String.fromCharCode(...new Uint8Array(buf)))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");

export function usePasskeyLogin(opts: {
  next: string;
  onError: (e: unknown) => void;
}) {
  const router = useRouter();
  return {
    runPasskeyLogin: useCallback(
      async (username: string) => {
        try {
          const begin = await api.post<{
            challenge: string;
            rp_id: string;
            cred_ids: string[];
          }>("/api/v1/auth/passkey-login/begin", { username });
          const allow = begin.cred_ids.map(
            (id): PublicKeyCredentialDescriptor => ({
              type: "public-key",
              id: Uint8Array.from(
                atob(id.replace(/-/g, "+").replace(/_/g, "/")),
                (ch) => ch.charCodeAt(0),
              ),
            }),
          );
          const assertion = (await navigator.credentials.get({
            publicKey: {
              challenge: new TextEncoder().encode(begin.challenge),
              rpId: begin.rp_id,
              ...(allow.length ? { allowCredentials: allow } : {}),
              userVerification: "preferred",
              timeout: 60_000,
            },
          })) as PublicKeyCredential | null;
          if (!assertion) throw new Error("浏览器取消了验证");
          const r = assertion.response as AuthenticatorAssertionResponse;
          const resp = await api.post<{
            must_reset_password: boolean;
          }>("/api/v1/auth/passkey-login/finish", {
            cred_id: B64URL(assertion.rawId),
            client_data_json: B64URL(r.clientDataJSON),
            auth_data: B64URL(r.authenticatorData),
            signature: B64URL(r.signature),
            sign_count: 0,
          });
          setSessionCookie(true);
          router.push(
            resp.must_reset_password ? "/my?tab=security" : opts.next,
          );
        } catch (err) {
          opts.onError(err);
          if (err instanceof ApiError && err.code === 2004)
            opts.onError(new Error("passkey attempt failed"));
        }
      },
      [router, opts],
    ),
  };
}
