"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import { PushSettings } from "@/components/push-settings";
import { TwoFactorSetup } from "@/components/twofa-setup";
import { ApiTokens } from "@/components/api-tokens";
import { LoginHistory } from "@/components/login-history";
import { SessionsCard } from "@/components/sessions-card";
import { NoticePrefsCard } from "@/components/notice-prefs";
import { PasskeysCard } from "@/components/passkeys-card";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
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
  // P2 清偿：重置 passkey 二次确认（danger：旧 RSS/下载链接全部失效）
  const ask = useConfirm();

  // 锚点直达：账户概览的「访问令牌 → 创建」带 #tokens、「密钥 → 重置」带
  // #passkey 跳进来。本面板是
  // 客户端 tab，浏览器不会自动滚到 tab 内的元素——挂载后手动定位并短暂
  // 高亮，让用户在平铺的十余项里一眼看到目标行。
  // ⚠️ 两处坑：① 必须分多拍补滚——面板内登录历史/活跃会话等子组件都异步
  //   取数，行高会随后续内容撑开，只滚一次会把目标行甩出视口（实测差
  //   348px）；② 高亮走 state 而非 classList.add——后者会被 React 的
  //   className 重渲染冲掉（实测 computed animationName 恒为 none）。
  const [anchor, setAnchor] = useState<string | null>(null);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const id = window.location.hash.slice(1);
    if (id !== "tokens" && id !== "passkey") return;
    const go = () =>
      document
        .getElementById(id)
        ?.scrollIntoView({ behavior: "smooth", block: "start" });
    go();
    setAnchor(id);
    const timers = [300, 900, 1800].map((ms) => setTimeout(go, ms));
    const off = setTimeout(() => setAnchor(null), 2600);
    return () => {
      timers.forEach(clearTimeout);
      clearTimeout(off);
    };
  }, []);
  const flash = (k: string) =>
    `scroll-mt-16${anchor === k ? " uc-anchor-flash" : ""}`;

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

  async function rotatePasskeyNow() {
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
          ? (apiErrorMessage(dict, e))
          : dict.common.networkError,
      );
    } finally {
      setPkBusy(false);
    }
  }

  return (
    <div className="baozi-wide-table-scroll">
      <table className="nexus-table nexus-form">
        <tbody>
          {/* 分组标题（2026-10-16）：本面板原先是十余项平铺一长表，从账户
              概览点「访问令牌 → 创建」进来根本找不到目标。按主题分四组，
              组标题走 colhead 跨列行，不改动任何一行的既有结构。 */}
          <tr><td className="colhead" colSpan={2}>{t.grpCredential}</td></tr>
          <Row
            head={t.resetPasskey}
            id="passkey"
            className={flash("passkey")}
          >
            <button
              type="button"
              disabled={pkBusy}
              onClick={() =>
                ask.open({
                  title: dict.my.passkeyRotate,
                  message: dict.my.passkeyConfirm,
                  tone: "danger",
                  busy: pkBusy,
                  run: rotatePasskeyNow,
                })
              }
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
                className="mt-2 min-h-[36px] rounded-full bg-sky-deep px-4 text-xs font-bold text-white disabled:opacity-50"
              >
                {pwBusy ? "…" : t.changeBtn}
              </button>
              {pwMsg && <p className="mt-1 text-xs text-sub">{pwMsg}</p>}
            </td>
          </tr>
          <Row head={t.passkeyLabel}>{t.passkeyHint}</Row>

          <tr><td className="colhead" colSpan={2}>{t.grpDevice}</td></tr>
          {/* 登录历史（NP 口径）：最近 20 条只读，异常登录一眼可见 */}
          <tr>
            <td className="rowhead nowrap">{t.loginHistory}</td>
            <td className="rowfollow p-0">
              <LoginHistory />
            </td>
          </tr>
          {/* 活跃会话 + 踢出其它会话（0284 P1-3） */}
          <tr>
            <td className="rowhead nowrap">{dict.sessions.title}</td>
            <td className="rowfollow p-0">
              <SessionsCard />
            </td>
          </tr>

          <tr><td className="colhead" colSpan={2}>{t.grpNotify}</td></tr>
          <tr>
            <td className="rowhead">{t.pushNotify}</td>
            <td className="rowfollow p-0">
              <PushSettings />
            </td>
          </tr>
          {/* 通知偏好（0075）：站内通知事件类开关 */}
          <tr>
            <td className="rowhead nowrap">{dict.noticePrefs.title}</td>
            <td className="rowfollow p-0">
              <NoticePrefsCard />
            </td>
          </tr>
          <Row head={t.tgBind}>{t.tgBindHint}</Row>

          <tr><td className="colhead" colSpan={2}>{t.grpApi}</td></tr>
          <tr id="tokens" className={flash("tokens")}>
            <td className="rowhead">{dict.apitokens.title}</td>
            <td className="rowfollow p-0">
              <ApiTokens />
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
      <AskDialog {...ask} />
    </div>
  );
}
