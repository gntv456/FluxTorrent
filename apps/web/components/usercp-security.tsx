"use client";

import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import { PushSettings } from "@/components/push-settings";
import { ApiTokens } from "@/components/api-tokens";
import { LoginHistory } from "@/components/login-history";
import { SessionsCard } from "@/components/sessions-card";
import { NoticePrefsCard } from "@/components/notice-prefs";
import { CredentialPanel } from "@/components/usercp-security-credential";
import { Tabs, TabPanel } from "@/components/ui/tabs";
import type { UserSettings } from "@/components/usercp";

/** 安全设定面板（从 usercp.tsx 按域拆出，300 行门禁）：
 *  重置 passkey / 两步验证 / 推送通知 / 登录历史 / 通知偏好 /
 *  API 令牌 / 修改密码 / 隐私等级。
 *
 *  2026-11-12：四组内容原先用 colhead 行竖排在同一张长表里，一屏看不全、
 *  还要滚两三屏。改为顶部标签切换（凭证与密码 / 登录与设备 / 通知 /
 *  开放 API 与隐私），一次只渲染一组，标签本身即分组标题。
 *  凭证与密码分区另拆到 usercp-security-credential.tsx（改密的三只输入
 *  框及其提交态只在该分区内用）。 */

/** 四个分区（顺序即标签顺序） */
type SecTab = "cred" | "device" | "notify" | "api";

/** 锚点 → 分区：账户概览的「访问令牌 → 创建」带 #tokens 来，落在开放
 *  API 那页；「密钥 → 重置」带 #passkey 来，落在凭证那页。 */
const ANCHOR_TAB: Record<string, SecTab> = {
  tokens: "api",
  passkey: "cred",
};

/** 一个分区 = 一张经典 nexus-form 表（行结构与改造前逐行一致） */
function SecPanel({
  tab,
  children,
}: {
  tab: SecTab;
  children: React.ReactNode;
}) {
  return (
    <TabPanel idPrefix="ucpsec" tabKey={tab}>
      <div className="brand-wide-table-scroll">
        <table className="nexus-table nexus-form">
          <tbody>{children}</tbody>
        </table>
      </div>
    </TabPanel>
  );
}

export function SecurityTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict } = useI18n();
  const t = dict.usercp.security;
  const [sec, setSec] = useState<SecTab>("cred");

  // 锚点直达：目标行现在可能在别的分区里，所以要先切分区、等它挂载，再滚。
  // ⚠️ 两处坑：① 必须分多拍补滚——分区是切过去的，首帧还没有该节点；且
  //   分区内的登录历史/活跃会话等子组件都异步取数，行高会随后续内容撑开，
  //   只滚一次会把目标行甩出视口（实测差 348px）；② 高亮走 state 而非
  //   classList.add——后者会被 React 的 className 重渲染冲掉（实测
  //   computed animationName 恒为 none）。
  const [anchor, setAnchor] = useState<string | null>(null);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const id = window.location.hash.slice(1);
    const target = ANCHOR_TAB[id];
    if (!target) return;
    setSec(target);
    setAnchor(id);
    const go = () =>
      document
        .getElementById(id)
        ?.scrollIntoView({ behavior: "smooth", block: "start" });
    const timers = [120, 600, 1200, 2000].map((ms) => setTimeout(go, ms));
    const off = setTimeout(() => setAnchor(null), 2800);
    return () => {
      timers.forEach(clearTimeout);
      clearTimeout(off);
    };
  }, []);
  const flash = (k: string) =>
    `scroll-mt-16${anchor === k ? " uc-anchor-flash" : ""}`;

  return (
    <div className="flex flex-col gap-3">
      <Tabs
        idPrefix="ucpsec"
        label={t.title}
        value={sec}
        onChange={(k) => setSec(k as SecTab)}
        items={[
          { key: "cred", label: t.grpCredential },
          { key: "device", label: t.grpDevice },
          { key: "notify", label: t.grpNotify },
          { key: "api", label: t.grpApi },
        ]}
      />

      {sec === "cred" && (
        <SecPanel tab="cred">
          <CredentialPanel flash={flash} />
        </SecPanel>
      )}

      {sec === "device" && (
        <SecPanel tab="device">
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
        </SecPanel>
      )}

      {sec === "notify" && (
        <SecPanel tab="notify">
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
        </SecPanel>
      )}

      {sec === "api" && (
        <SecPanel tab="api">
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
        </SecPanel>
      )}
    </div>
  );
}
