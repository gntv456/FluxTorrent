"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import { PushSettings } from "@/components/push-settings";
import { TwoFactorSetup } from "@/components/twofa-setup";
import { ApiTokens } from "@/components/api-tokens";
import { LoginHistory } from "@/components/login-history";
import { NoticePrefsCard } from "@/components/notice-prefs";
import type { UserSettings } from "@/components/usercp";

/** 安全设定面板（从 usercp.tsx 按域拆出，300 行门禁）：
 *  重置 passkey / 两步验证 / 推送通知 / 登录历史 / 通知偏好 /
 *  API 令牌 / 修改密码 / 隐私等级。 */

export function SecurityTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.usercp.security;
  const [newPass, setNewPass] = useState("");
  const [newPass2, setNewPass2] = useState("");
  const [oldPass, setOldPass] = useState("");
  const [pwBusy, setPwBusy] = useState(false);
  const [pwMsg, setPwMsg] = useState<string | null>(null);
  const [pkBusy, setPkBusy] = useState(false);
  const [pkResult, setPkResult] = useState<string | null>(null);
  const [pkMsg, setPkMsg] = useState<string | null>(null);

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
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setPwBusy(false);
    }
  }

  async function rotatePasskeyNow() {
    if (!window.confirm(dict.my.passkeyConfirm)) return;
    setPkBusy(true);
    setPkMsg(null);
    try {
      const r = await api.post<{ passkey: string }>(
        "/api/v1/me/passkey/rotate",
        {},
      );
      setPkResult(r.passkey);
      setPkMsg(dict.my.passkeyRotated);
    } catch (e) {
      setPkMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setPkBusy(false);
    }
  }

  return (
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.resetPasskey}>
          <button
            type="button"
            disabled={pkBusy}
            onClick={rotatePasskeyNow}
            className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white disabled:opacity-50"
          >
            {pkBusy ? dict.my.passkeyRotating : dict.my.passkeyRotate}
          </button>
          {pkResult && (
            <>
              <br />
              <code className="num break-all text-xs">{pkResult}</code>
            </>
          )}
          {pkMsg && <p className="text-xs text-sub">{pkMsg}</p>}
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
        <tr>
          <td className="rowhead">{t.pushNotify}</td>
          <td className="rowfollow p-0">
            <PushSettings />
          </td>
        </tr>
        {/* 登录历史（NP 口径）：最近 20 条只读，异常登录一眼可见 */}
        <tr>
          <td className="rowhead nowrap">{t.loginHistory}</td>
          <td className="rowfollow p-0">
            <LoginHistory />
          </td>
        </tr>
        {/* 通知偏好（0075）：站内通知事件类开关 */}
        <tr>
          <td className="rowhead nowrap">{dict.noticePrefs.title}</td>
          <td className="rowfollow p-0">
            <NoticePrefsCard />
          </td>
        </tr>
        <Row head={t.passkeyLabel}>{t.passkeyHint}</Row>
        <Row head={t.tgBind}>{t.tgBindHint}</Row>
        <tr>
          <td className="rowhead">{dict.apitokens.title}</td>
          <td className="rowfollow p-0">
            <ApiTokens />
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
              className="mt-2 min-h-[36px] rounded-full bg-sky-deep px-4 text-xs font-bold text-white disabled:opacity-50"
            >
              {pwBusy ? "…" : t.changeBtn}
            </button>
            {pwMsg && <p className="mt-1 text-xs text-sub">{pwMsg}</p>}
          </td>
        </tr>
        <Row head={t.privacy}>
          <label>
            <input
              type="radio"
              name="privacy"
              checked={s.privacy === "normal"}
              onChange={() => patch({ privacy: "normal" })}
            />{" "}
            {t.privacyNormal}
          </label>
          <label>
            <input
              type="radio"
              name="privacy"
              checked={s.privacy === "low"}
              onChange={() => patch({ privacy: "low" })}
            />{" "}
            {t.privacyLow}
          </label>
          <label>
            <input
              type="radio"
              name="privacy"
              checked={s.privacy === "strong"}
              onChange={() => patch({ privacy: "strong" })}
            />{" "}
            {t.privacyStrong}
          </label>
        </Row>
      </tbody>
    </table>
  );
}
