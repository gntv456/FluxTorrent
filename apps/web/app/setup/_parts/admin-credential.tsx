"use client";

import { useEffect, useState } from "react";
import type { Status } from "./setup-types";

/**
 * 向导第二步的管理员凭据区（从 step-site.tsx 按域拆出，300 行门禁）。
 *
 * 存在的理由（用户实测困惑）：改密块（新密码/确认新密码）与下方
 * 「管理员用户名 / 管理员密码」两组框**并列出现**，且两者语义相反——
 * 上面是「设新密码」，下面是「填旧密码登录」。站长无从判断该填哪个，
 * 填错即「凭证无效」。
 *
 * 但 needReset 场景下，下面那两个框要填的东西**全是系统已知常量**
 * （0017 引导的 root + password123），让站长凭空填一遍毫无必要，
 * 于是这里按 status 收敛成**一个概念、一个动作**：
 *   - 临时密码 + 仍是引导默认口令 → 只让站长设新密码（账号/旧口令
 *     自动填，不渲染出来）；
 *   - 临时密码但口令已被重置（ipcheck 发的随机临时串，站点长只知道
 *     它是哈希）→ 必须手填当前密码，并说明它是「改密前的那串」；
 *   - 非临时密码（老站重入）→ 就是普通登录，措辞明确写「登录」。
 */

export const inputCls =
  "w-full rounded-md border border-line bg-[var(--field-bg)] px-3 py-2 " +
  "text-sm outline-none focus:border-accent";

/** 父组件持有的凭据值（needReset 时 password 是旧口令、newPassword 是新设） */
export interface Credential {
  username: string;
  password: string;
  newPassword: string;
  /** 新密码的第二次输入（仅本地校验用，不外传） */
  newPassword2: string;
}

/** 引导默认口令（明文 password123）。仅在 status 确认哈希未变时自动填 */
const BOOTSTRAP_DEFAULT_PW = "password123";
export function SetupAdminCredential({
  t,
  status,
  value,
  onChange,
}: {
  t: (k: string, fallback: string) => string;
  status: Status | null;
  value: Credential;
  onChange: (c: Credential) => void;
}) {
  const [localErr, setLocalErr] = useState("");
  const needReset = status?.root_temp_password === true;
  // 「旧口令仍是引导默认值」由后端哈希比对给出（见 status.rs），
  // 不按用户名 == root 假定——那会让被重置过口令的站静默用错
  const autoFill = needReset && status?.temp_admin_default_pw === true;
  const presetUser = status?.temp_admin_username ?? "";

  // 系统已知的凭据自动填入（仅一次：status 到达后）
  useEffect(() => {
    if (!needReset) return;
    const patch: Partial<Credential> = {};
    if (presetUser && !value.username) patch.username = presetUser;
    if (autoFill && !value.password) patch.password = BOOTSTRAP_DEFAULT_PW;
    if (Object.keys(patch).length > 0) onChange({ ...value, ...patch });
    // 只在系统已知凭据到达时填一次：value 故意不进依赖（它每次渲染都是
    // 新对象，进去会反复触发 onChange → 死循环）
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [needReset, autoFill, presetUser]);

  const set = (patch: Partial<Credential>) => onChange({ ...value, ...patch });

  /** 校验：把 mismatch 的即时反馈挂在 blur 上（失焦即提示，不等点按钮） */
  function validate() {
    if (!needReset) return true;
    if (value.newPassword.length > 0 && value.newPassword.length < 8) {
      setLocalErr(t("pwTooShort", "新密码至少 8 位"));
      return false;
    }
    if (
      value.newPassword2.length > 0 &&
      value.newPassword !== value.newPassword2
    ) {
      setLocalErr(t("pwMismatch", "两次输入的新密码不一致"));
      return false;
    }
    setLocalErr("");
    return true;
  }

  return (
    <div className="space-y-3">
      {needReset ? (
        <>
          <div
            className="rounded-md border border-warn/40 bg-warn/10 px-3
              py-2 text-xs"
          >
            <p className="font-medium">
              {t("credResetTitle", "① 先给管理员设置新密码")}
            </p>
            <p className="mt-1 text-muted">
              {autoFill
                ? t(
                    "credResetIntroAuto",
                    "系统已预置管理员账号，且仍在使用初始密码 password123" +
                      "。临时密码下除改密外的接口都会被拦截，不改密就" +
                      "无法完成安装。",
                  )
                : t(
                    "credResetIntroManual",
                    "系统预置管理员账号的密码已被重置为随机临时密码，" +
                      "请先输入它以完成改密。",
                  )}
            </p>
          </div>
          {/* 已知的账号与旧口令不渲染成输入框——它们是系统常量，
              让站长填一遍只会引发「该填哪个」的困惑 */}
          <div className="rounded-md bg-[var(--panel)] px-3 py-2 text-xs">
            <div className="flex justify-between gap-2">
              <span className="text-muted">
                {t("credAdminUser", "管理员账号")}
              </span>
              <span className="font-mono">{presetUser || "—"}</span>
            </div>
            {autoFill && (
              <div className="mt-1 flex justify-between gap-2">
                <span className="text-muted">
                  {t("credCurrentPw", "当前密码")}
                </span>
                <span className="font-mono">
                  {t("credFilledAuto", "已自动填入，无需修改")}
                </span>
              </div>
            )}
          </div>
          {!autoFill && (
            <label className="block text-sm">
              <span className="text-muted">
                {t("credCurrentPw", "当前密码")}
              </span>
              <input
                type="password"
                className={`mt-1 ${inputCls}`}
                value={value.password}
                onChange={(e) => set({ password: e.target.value })}
                autoComplete="current-password"
              />
              <span className="mt-1 block text-xs text-muted">
                {t(
                  "credCurrentPwHint",
                  "即重置密码时显示给你的那串临时密码——它将被新密码替换。",
                )}
              </span>
            </label>
          )}
          <label className="block text-sm">
            <span className="text-muted">
              {t("newPw", "新密码（至少 8 位）")}
            </span>
            <input
              type="password"
              className={`mt-1 ${inputCls}`}
              value={value.newPassword}
              onChange={(e) => set({ newPassword: e.target.value })}
              onBlur={validate}
              autoComplete="new-password"
            />
            <span className="mt-1 block text-xs text-muted">
              {t(
                "credNewPwHint",
                "这就是你之后登录本站用的密码，请立即牢记——忘记后只能" +
                  "走数据库重置。",
              )}
            </span>
          </label>
          <label className="block text-sm">
            <span className="text-muted">
              {t("newPw2", "再输一次新密码")}
            </span>
            <input
              type="password"
              className={`mt-1 ${inputCls}`}
              value={value.newPassword2}
              onChange={(e) => set({ newPassword2: e.target.value })}
              onBlur={validate}
              autoComplete="new-password"
            />
          </label>
          <p className="text-xs text-muted">
            {t(
              "credResetFoot",
              "改密会同时注销该账号在其他设备上的登录；本站目前只有这" +
                "一个管理员，改密后请用新密码登录。",
            )}
          </p>
        </>
      ) : (
        <>
          <p className="text-sm">
            {t("credLoginTitle", "② 用已有管理员账号登录")}
          </p>
          <p className="text-xs text-muted">
            {t(
              "credLoginHint",
              "本站已存在管理员。请填它的账号密码，向导用它完成最后一步" +
                "安装确认。",
            )}
          </p>
          <label className="block text-sm">
            <span className="text-muted">
              {t("adminUser", "管理员用户名")}
            </span>
            <input
              className={`mt-1 ${inputCls}`}
              value={value.username}
              onChange={(e) => set({ username: e.target.value })}
              autoComplete="username"
            />
          </label>
          <label className="block text-sm">
            <span className="text-muted">
              {t("adminPass", "管理员密码")}
            </span>
            <input
              type="password"
              className={`mt-1 ${inputCls}`}
              value={value.password}
              onChange={(e) => set({ password: e.target.value })}
              autoComplete="current-password"
            />
          </label>
        </>
      )}
      {localErr && (
        <p
          className="rounded-md border border-danger/40 bg-danger/10 px-3
            py-2 text-sm text-danger"
        >
          {localErr}
        </p>
      )}
    </div>
  );
}
