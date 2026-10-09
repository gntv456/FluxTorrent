"use client";

import { useEffect, useState } from "react";
import { api, ApiError, setSessionCookie } from "@/lib/api-client";
import type { Status } from "./setup-types";
import {
  SetupAdminCredential,
  inputCls,
  type Credential,
} from "./admin-credential";

/**
 * 向导第二步：站点名 + 管理员凭据 + announce（G12）。
 *
 * 临时密码管理员在此就地改密：must_reset 闸门只放行改密/登出/自身信息，
 * 不先改密 POST /setup 必被拦（G12 复验发现，见方案执行记录）；
 * 改密会撤销既有 token，故带新密码交回主组件重新登录完成安装。
 *
 * 凭据区已拆至 ./_parts/admin-credential.tsx：此前「新密码/确认新密码」
 * 与「管理员用户名/管理员密码」两组语义相反的框并列，站长无从判断
 * 该填哪个（实测困惑），而 needReset 时后两者其实是系统已知常量。
 * 从 setup/page.tsx 按域拆出（300 行门禁）。
 */

export interface SiteDraft {
  siteName: string;
  username: string;
  password: string;
  announceUrl: string;
}

export function SetupStepSite({
  t,
  btn,
  status,
  initial,
  onBack,
  onNext,
}: {
  t: (k: string, fallback: string) => string;
  btn: string;
  status: Status | null;
  initial: SiteDraft;
  onBack: () => void;
  onNext: (d: SiteDraft) => void;
}) {
  const [siteName, setSiteName] = useState(initial.siteName);
  const [announceUrl, setAnnounceUrl] = useState(initial.announceUrl);
  const [cred, setCred] = useState<Credential>({
    username: initial.username,
    password: initial.password,
    newPassword: "",
    newPassword2: "",
  });
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const needReset = status?.root_temp_password === true;
  // 从浏览器地址推导建议 announce：有域名（非 IP/localhost）才建议——
  // 冷启动经 IP:3000 访问时不猜（此时反代/证书都没配，填了也不通），
  // 留空 + 收尾指引比一个错值更安全。
  const [suggest, setSuggest] = useState("");
  useEffect(() => {
    if (typeof window === "undefined") return;
    const host = window.location.hostname;
    const isIp =
      /^\d+\.\d+\.\d+\.\d+$/.test(host) || host.includes(":");
    const isLocal =
      host === "localhost" || host.endsWith(".local");
    if (!isIp && !isLocal && host.includes(".")) {
      setSuggest(`https://${host}`);
    }
  }, []);

  async function next() {
    setErr("");
    if (!cred.username || !cred.password) {
      setErr(t("credMissing", "请填写管理员账号与密码"));
      return;
    }
    // 非临时密码场景就是普通登录，新密码为空即当前口令
    if (!needReset) {
      onNext({
        siteName,
        username: cred.username,
        password: cred.password,
        announceUrl,
      });
      return;
    }
    if (cred.newPassword.length < 8) {
      setErr(t("pwTooShort", "新密码至少 8 位"));
      return;
    }
    // 二次确认也要在这里把关：子组件的 blur 校验只是即时反馈，
    // 点「下一步」不触发 blur，两处不一致会放过不匹配的密码
    if (cred.newPassword !== cred.newPassword2) {
      setErr(t("pwMismatch", "两次输入的新密码不一致"));
      return;
    }
    setBusy(true);
    try {
      await api.post("/api/v1/auth/login", {
        username: cred.username,
        password: cred.password,
      });
      await api.post("/api/v1/me/password/change", {
        old_password: cred.password,
        new_password: cred.newPassword,
      });
      setSessionCookie(true);
      setBusy(false);
      // 交回新口令：主组件用它重新登录完成安装
      onNext({
        siteName,
        username: cred.username,
        password: cred.newPassword,
        announceUrl,
      });
    } catch (e) {
      setBusy(false);
      // 「凭证无效」是登录端点的通用兜底文案，对「当前密码填错」毫无指示性。
      // 向导第二步手填旧口令时填错是高频操作（且用户无从确认是哪一栏错），
      // 这里就地补一句指路，而不是让人对着一句密码错误猜。
      const raw = e instanceof ApiError ? e.message : String(e);
      setErr(
        /凭证无效|Invalid credentials/i.test(raw)
          ? t(
              "pwOldWrong",
              "当前密码不正确——请填重置密码时显示给你的那串临时密码。" +
                "（新密码可以随便改，这一栏要填的是改密前那串）",
            )
          : raw,
      );
    }
  }

  const blocked =
    !cred.username ||
    !cred.password ||
    busy ||
    (needReset &&
      (cred.newPassword.length < 8 ||
        cred.newPassword !== cred.newPassword2));

  return (
    <div className="mt-6 space-y-4">
      <label className="block text-sm">
        <span className="text-muted">{t("siteName", "站点名称")}</span>
        <input
          className={`mt-1 ${inputCls}`}
          value={siteName}
          onChange={(e) => setSiteName(e.target.value)}
          placeholder={status?.site_name || "例：枫叶 PT"}
        />
        {/* G12：可留空要写明——灰色占位符就是留空后的实际站名 */}
        <span className="mt-1 block text-xs text-muted">
          {t(
            "siteNameHint",
            "可留空——留空将沿用当前站名（见输入框灰色提示）；之后可在" +
              "后台「站点设定 → 基础设定」修改。",
          )}
        </span>
      </label>

      <SetupAdminCredential
        t={t}
        status={status}
        value={cred}
        onChange={setCred}
      />

      {err && (
        <p
          className="rounded-md border border-danger/40 bg-danger/10 px-3
            py-2 text-sm text-danger"
        >
          {err}
        </p>
      )}

      {/* P0-2.1：announce 在向导内采集。留空 = 保持现状（幂等重入不炸
          老站），但公网填写的引导文案到位；仍填 127.0.0.1 由后端拒绝。
          有域名访问时给出「一键采用建议值」；IP 访问（冷启动常态）时
          明确告知先留空、收尾再配 */}
      <label className="block text-sm">
        <span className="text-muted">
          {t("announceUrl", "Tracker 公网地址（announce URL）")}
        </span>
        <input
          className={`mt-1 ${inputCls}`}
          value={announceUrl}
          onChange={(e) => setAnnounceUrl(e.target.value)}
          placeholder={suggest || "https://tracker.example.com"}
        />
        {suggest ? (
          <span className="mt-1 block text-xs text-muted">
            {t("announceSuggestPrefix", "检测到你在用域名访问：")}
            <button
              type="button"
              className="mx-1 underline"
              onClick={() => setAnnounceUrl(suggest)}
            >
              {t("announceSuggestUse", `采用 ${suggest}`)}
            </button>
            {t(
              "announceSuggestSuffix",
              "（前提：HTTPS 已配好；还没配就先留空，配完在后台填）",
            )}
          </span>
        ) : (
          <span className="mt-1 block text-xs text-muted">
            {t(
              "announceHint",
              "这是你站点的公网 Tracker 地址——其他用户下载种子后将通" +
                "过它连接做种。留空保持现状；填 127.0.0.1/localhost 将被" +
                "拒绝。",
            )}
            {t(
              "announceHintColdstart",
              "你现在是 IP 访问（域名/HTTPS 还没配）：建议先留空，配好" +
                "域名后到后台「站点设定 → 基础设定 → Tracker 地址」填 " +
                "https://你的域名。",
            )}
          </span>
        )}
      </label>
      <div className="flex gap-2">
        <button className={`${btn}`} onClick={onBack}>
          {t("prev", "上一步")}
        </button>
        <button className={btn} disabled={blocked} onClick={next}>
          {busy ? t("working", "执行中…") : t("next", "下一步")}
        </button>
      </div>
    </div>
  );
}
