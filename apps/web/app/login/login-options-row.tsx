"use client";

import { useI18n } from "@/i18n/client";

/** 登录页「记住登录 + 两步验证」行（从 login-form 按域拆出，300 行门禁）。
 *
 *  状态仍归 LoginForm 持有：提交载荷要读 remember，2010/2011 两个错误码
 *  要自动展开验证码框——放在本组件里就够不着了，所以这里只做受控展示。
 *
 *  remember 在 2026-11-01 之前是纯装饰（无 state、不进请求体、后端
 *  LoginReq 也没这个字段），现在是真的：勾上 = 会话拉到 30 天。 */
export function LoginOptionsRow({
  remember,
  onRemember,
  showTotp,
  onToggleTotp,
}: {
  remember: boolean;
  onRemember: (v: boolean) => void;
  showTotp: boolean;
  onToggleTotp: () => void;
}) {
  const { dict } = useI18n();
  return (
    <div className="bz-options">
      <label className="bz-remember">
        <input
          type="checkbox"
          checked={remember}
          onChange={(e) => onRemember(e.target.checked)}
        />
        <span className="bz-checkbox-mark" aria-hidden />
        {dict.login.remember}
      </label>
      <button
        type="button"
        onClick={onToggleTotp}
        aria-expanded={showTotp}
        title={dict.login.twoStepTooltip}
        className="bz-totp-toggle"
      >
        {dict.login.twoStepCode}
      </button>
    </div>
  );
}
