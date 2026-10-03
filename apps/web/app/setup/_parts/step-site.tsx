"use client";

import { useEffect, useState } from "react";
import { api, ApiError, setSessionCookie } from "@/lib/api-client";
import type { Status } from "./setup-types";

/**
 * 向导第二步：站点名 + 管理员登录 + announce（G12）。
 * 临时密码管理员在此就地改密：must_reset 闸门只放行改密/登出/自身信息，
 * 不先改密 POST /setup 必被拦（G12 复验发现，见方案执行记录）；
 * 改密会撤销既有 token，故带新密码交回主组件重新登录完成安装。
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
  const [username, setUsername] = useState(initial.username);
  const [password, setPassword] = useState(initial.password);
  const [announceUrl, setAnnounceUrl] = useState(initial.announceUrl);
  const [newPw, setNewPw] = useState("");
  const [newPw2, setNewPw2] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  // 仅当后台上报「引导 root 仍是临时密码」时渲染改密块
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
    if (!needReset) {
      onNext({ siteName, username, password, announceUrl });
      return;
    }
    if (newPw.length < 8) {
      setErr(t("pwTooShort", "新密码至少 8 位"));
      return;
    }
    if (newPw !== newPw2) {
      setErr(t("pwMismatch", "两次输入的新密码不一致"));
      return;
    }
    setBusy(true);
    try {
      await api.post("/api/v1/auth/login", { username, password });
      await api.post("/api/v1/me/password/change", {
        old_password: password,
        new_password: newPw,
      });
      setSessionCookie(true);
      setBusy(false);
      onNext({ siteName, username, password: newPw, announceUrl });
    } catch (e) {
      setBusy(false);
      setErr(e instanceof ApiError ? e.message : String(e));
    }
  }

  const inputCls =
    "w-full rounded-md border border-line bg-[var(--field-bg)] px-3 py-2 " +
    "text-sm outline-none focus:border-accent";

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
      {needReset && (
        <div
          className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2
            text-xs"
        >
          <p className="font-medium">
            {t(
              "needNewPwTitle",
              "管理员正在使用初始密码：先设置新密码再继续",
            )}
          </p>
          <p className="mt-1 text-muted">
            {t(
              "needNewPwHint",
              "系统预置管理员 root（初始密码 password123）为临时密码—" +
                "—临时密码下除改密外的接口都会被拦截，不先改密无法完成" +
                "安装。设置后向导会直接用新密码完成安装（其他设备需重" +
                "新登录）。",
            )}
          </p>
          <label className="mt-2 block">
            <span className="text-muted">
              {t("newPw", "新密码（至少 8 位）")}
            </span>
            <input
              type="password"
              className={`mt-1 ${inputCls}`}
              value={newPw}
              onChange={(e) => setNewPw(e.target.value)}
              autoComplete="new-password"
            />
          </label>
          <label className="mt-2 block">
            <span className="text-muted">{t("newPw2", "确认新密码")}</span>
            <input
              type="password"
              className={`mt-1 ${inputCls}`}
              value={newPw2}
              onChange={(e) => setNewPw2(e.target.value)}
              autoComplete="new-password"
            />
          </label>
        </div>
      )}
      {err && (
        <p
          className="rounded-md border border-danger/40 bg-danger/10 px-3
            py-2 text-sm text-danger"
        >
          {err}
        </p>
      )}
      <label className="block text-sm">
        <span className="text-muted">{t("adminUser", "管理员用户名")}</span>
        <input
          className={`mt-1 ${inputCls}`}
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
        />
      </label>
      <label className="block text-sm">
        <span className="text-muted">{t("adminPass", "管理员密码")}</span>
        <input
          type="password"
          className={`mt-1 ${inputCls}`}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
        />
      </label>
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
        <button
          className={btn}
          disabled={
            !username ||
            !password ||
            busy ||
            (needReset && (!newPw || !newPw2))
          }
          onClick={next}
        >
          {busy ? t("working", "执行中…") : t("next", "下一步")}
        </button>
      </div>
    </div>
  );
}
