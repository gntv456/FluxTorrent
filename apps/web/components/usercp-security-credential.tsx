"use client";

/** 安全设定 · 凭证与密码分区（从 usercp-security.tsx 按域外移）：
 *  重置 passkey / 两步验证 / Passkey 绑定 / 自助改密 / 通行密钥说明。
 *
 *  改密的三只输入框与提交态随本件一起搬走——它们只在这一分区内使用，
 *  留在外层会让 SecurityTab 为了别的分区也持有五个无用 state。 */

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import { TwoFactorSetup } from "@/components/twofa-setup";
import { PasskeysCard } from "@/components/passkeys-card";
import { PasskeyRotate } from "@/components/passkey-rotate";
import { BTN_SM_SKY } from "@/lib/ui-classes";

export function CredentialPanel({ flash }: { flash: (k: string) => string }) {
  const { dict } = useI18n();
  const t = dict.usercp.security;
  const [newPass, setNewPass] = useState("");
  const [newPass2, setNewPass2] = useState("");
  const [oldPass, setOldPass] = useState("");
  const [pwBusy, setPwBusy] = useState(false);
  const [pwMsg, setPwMsg] = useState<string | null>(null);

  async function changePasswordNow() {
    setPwMsg(null);
    if (newPass.length < 8) {
      setPwMsg(dict.usercp.security.pwTooShort);
      return;
    }
    if (newPass !== newPass2) {
      setPwMsg(dict.usercp.security.pwMismatch);
      return;
    }
    setPwBusy(true);
    try {
      await api.post("/api/v1/me/password/change", {
        old_password: oldPass,
        new_password: newPass,
      });
      setPwMsg(dict.usercp.security.pwChanged);
      setOldPass("");
      setNewPass("");
      setNewPass2("");
    } catch (e) {
      setPwMsg(
        e instanceof ApiError
          ? (apiErrorMessage(dict, e))
          : dict.common.networkError,
      );
    } finally {
      setPwBusy(false);
    }
  }

  return (
    <>
      <Row
        head={t.resetPasskey}
        id="passkey"
        className={flash("passkey")}
      >
        <PasskeyRotate />
        <br />
        <span className="uc-note">
          <b>{dict.usercp.note}</b>
          {t.resetPasskeyNote}
        </span>
      </Row>
      <Row head={t.twoStep}>
        <TwoFactorSetup />
        <br />
        {t.twoStepHint}
      </Row>
      {/* Passkey（WebAuthn）绑定（0227） */}
      <tr>
        <td className="rowhead nowrap">Passkey</td>
        <td className="rowfollow p-0">
          <PasskeysCard />
        </td>
      </tr>
      <tr>
        <td className="rowhead nowrap">{t.oldPassword}</td>
        <td className="rowfollow">
          <input
            type="password"
            className="uc-password"
            autoComplete="current-password"
            value={oldPass}
            onChange={(e) => setOldPass(e.target.value)}
          />
        </td>
      </tr>
      <tr>
        <td className="rowhead nowrap">{t.newPassword}</td>
        <td className="rowfollow">
          <input
            type="password"
            className="uc-password"
            autoComplete="new-password"
            value={newPass}
            onChange={(e) => setNewPass(e.target.value)}
          />
        </td>
      </tr>
      <tr>
        <td className="rowhead nowrap">{t.confirmPassword}</td>
        <td className="rowfollow">
          <input
            type="password"
            className="uc-password"
            autoComplete="new-password"
            value={newPass2}
            onChange={(e) => setNewPass2(e.target.value)}
          />
          <br />
          <button
            type="button"
            disabled={pwBusy || !oldPass || !newPass || !newPass2}
            onClick={changePasswordNow}
            className={`mt-2 ${BTN_SM_SKY}`}
          >
            {pwBusy ? "…" : t.changeBtn}
          </button>
          {pwMsg && <p className="mt-1 text-xs text-sub">{pwMsg}</p>}
        </td>
      </tr>
      <Row head={t.passkeyLabel}>{t.passkeyHint}</Row>
    </>
  );
}
