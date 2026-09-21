"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 安全中心 2FA 自助绑定/解绑（TOTP RFC 6238）：setup → 扫码/手抄密钥 → enable；已启用可 disable */
export function TwoFactorSetup() {
  const { dict } = useI18n();
  const t = dict.security2fa ?? {
    statusOn: "已启用",
    statusOff: "未启用",
    statusLoading: "查询中…",
    setup: "生成 2FA 密钥",
    secretLabel: "密钥（验证器手动输入用）",
    codeLabel: "验证器 6 位动态码",
    enable: "确认启用",
    disable: "关闭两步验证",
    enabledOk: "两步验证已启用",
    disabledOk: "两步验证已关闭",
  };

  // enabled 状态三态：null=未知（无专用查询接口，enable/setup 报错即未启用）
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [setup, setSetup] = useState<{
    secret: string;
    otpauth_uri: string;
  } | null>(null);
  const [code, setCode] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const beginSetup = useCallback(async () => {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ secret: string; otpauth_uri: string }>(
        "/api/v1/me/2fa/setup",
        {},
      );
      setSetup(r);
    } catch (e) {
      // 已启用的用户 setup 会报"2FA 已启用"
      setMsg(apiErrorMessage(dict, e));
      setEnabled(true);
    } finally {
      setBusy(false);
    }
  }, [dict]);

  useEffect(() => {
    // /me/overview 的 totp_enabled 字段探测启用态
    api
      .get<{ totp_enabled?: boolean }>("/api/v1/me/overview")
      .then((s) => setEnabled(Boolean(s.totp_enabled)))
      .catch(() => setEnabled(null));
  }, []);

  async function enable() {
    const c = parseInt(code, 10);
    if (!Number.isFinite(c) || code.length !== 6) {
      setMsg(t.codeLabel);
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/me/2fa/enable", { code: c });
      setEnabled(true);
      setSetup(null);
      setCode("");
      setMsg(t.enabledOk);
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  async function disable() {
    const c = parseInt(code, 10);
    if (!Number.isFinite(c) || code.length !== 6) {
      setMsg(t.codeLabel);
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/me/2fa/disable", { code: c });
      setEnabled(false);
      setCode("");
      setMsg(t.disabledOk);
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2 text-sm">
      <p>
        {enabled === null
          ? t.statusLoading
          : enabled
            ? `✅ ${t.statusOn}`
            : `⚪ ${t.statusOff}`}
      </p>

      {!enabled && !setup && (
        <button
          type="button"
          disabled={busy}
          onClick={beginSetup}
          className="min-h-[36px] w-fit rounded-full bg-sky px-4 text-xs font-bold text-white disabled:opacity-50"
        >
          {t.setup}
        </button>
      )}

      {setup && (
        <div className="flex flex-col gap-1">
          <p className="text-xs text-sub">{t.secretLabel}：</p>
          <code className="num break-all rounded-[var(--r-sm)] bg-cloud px-2 py-1 text-xs">
            {setup.secret}
          </code>
          <code className="break-all text-[10px] text-sub">
            {setup.otpauth_uri}
          </code>
          <div className="mt-1 flex flex-wrap items-center gap-2">
            <input
              value={code}
              onChange={(e) =>
                setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
              }
              inputMode="numeric"
              placeholder={t.codeLabel}
              aria-label={t.codeLabel}
              className="num w-28 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2 py-1 text-sm"
            />
            <button
              type="button"
              disabled={busy}
              onClick={enable}
              className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white disabled:opacity-50"
            >
              {t.enable}
            </button>
          </div>
        </div>
      )}

      {enabled && (
        <div className="flex flex-wrap items-center gap-2">
          <input
            value={code}
            onChange={(e) =>
              setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
            }
            inputMode="numeric"
            placeholder={t.codeLabel}
            aria-label={t.codeLabel}
            className="num w-28 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2 py-1 text-sm"
          />
          <button
            type="button"
            disabled={busy}
            onClick={disable}
            className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger disabled:opacity-50"
          >
            {t.disable}
          </button>
        </div>
      )}

      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}
    </div>
  );
}
