"use client";

import { BTN_SM_SKY } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

/** 开放 API 令牌自助管理（M27）：签发（明文一次性显示）/ 列表 / 吊销 */
interface TokenRow {
  id: number;
  name: string;
  scopes: string[];
  rate_per_min: number;
  last_used_at: string | null;
  revoked_at: string | null;
  created_at: string;
}

export function ApiTokens() {
  const { dict } = useI18n();
  const t = dict.apitokens ?? {
    title: "开放 API 令牌",
    namePlaceholder: "令牌名称（如：RSS 同步）",
    issue: "签发令牌",
    revoke: "吊销",
    revoked: "已吊销",
    neverUsed: "未使用",
    plainOnce: "令牌明文仅此一次显示，请立即保存：",
    limit: "每人最多 3 枚有效令牌",
    stepUpHint: "",
  };
  // 输入框文案直接复用安全面板那组（字面量本身就是通用的「当前密码」
  // 「两步验证码」），免得同一句话在三语字典里各写两份。
  const sec = dict.usercp.security;
  const [rows, setRows] = useState<TokenRow[]>([]);
  const [name, setName] = useState("");
  const [plain, setPlain] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // P-113：Token 是 180 天的长期凭证，改密也不会让它失效，服务端现在要
  // 复核凭证才签。启用态探测沿用 twofa-setup / passkeys-card 的同一口径
  // （/me/overview 的 totp_enabled），服务端仍会自己再核一遍。
  const [pw, setPw] = useState("");
  const [code, setCode] = useState("");
  const [twofa, setTwofa] = useState(false);

  // P-314：本表没有任何空态文案，取数失败与「你没配过 Token」在界面上是同一个
  // 空壳（凭证面）——补一条显式失败提示。
  const [failed, setFailed] = useState(false);
  const load = useCallback(async () => {
    setFailed(false);
    try {
      setRows(await api.get<TokenRow[]>("/api/v1/me/tokens"));
    } catch {
      setRows([]);
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    load();
    void api
      .get<{ totp_enabled?: boolean }>("/api/v1/me/overview")
      .then((o) => setTwofa(Boolean(o?.totp_enabled)))
      .catch(() => setTwofa(false));
  }, [load]);

  async function issue() {
    if (!name.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ token: string }>("/api/v1/me/tokens", {
        name: name.trim(),
        password: pw,
        ...(twofa ? { totp: Number(code) } : {}),
      });
      setPlain(r.token);
      setName("");
      setPw("");
      setCode("");
      load();
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  async function revoke(id: number) {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/me/tokens/revoke", { id });
      load();
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2 text-sm">
      {failed && (
        <p role="alert" className="text-xs text-sub">
          {dict.common.loadFailed}
        </p>
      )}
      <p className="text-xs text-sub">{t.limit}</p>
      {t.stepUpHint && (
        <p className="text-xs text-sub">{t.stepUpHint}</p>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t.namePlaceholder}
          aria-label={t.namePlaceholder}
          className="min-h-[36px] w-56 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 text-sm"
        />
        <input
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          type="password"
          autoComplete="current-password"
          aria-label={sec.webauthnPwPh}
          placeholder={sec.webauthnPwPh}
          className="min-h-[36px] w-40 rounded-[var(--r-sm)] border
            border-line bg-[var(--surface-card)] px-3 text-sm"
        />
        {twofa && (
          <input
            value={code}
            onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))}
            inputMode="numeric"
            maxLength={6}
            autoComplete="one-time-code"
            aria-label={sec.webauthnCodePh}
            placeholder={sec.webauthnCodePh}
            className="min-h-[36px] w-32 rounded-[var(--r-sm)] border
              border-line bg-[var(--surface-card)] px-3 text-sm"
          />
        )}
        <button
          type="button"
          disabled={busy || !name.trim() || !pw || (twofa && code.length < 6)}
          onClick={issue}
          className={BTN_SM_SKY}
        >
          {t.issue}
        </button>
      </div>

      {plain && (
        <p className="rounded-[var(--r-md)] bg-sun/30 p-2 text-xs">
          {t.plainOnce}
          <code className="num ml-1 break-all font-bold">{plain}</code>
        </p>
      )}
      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}

      {rows.length > 0 && (
        <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{t.title}</td>
              <td className="colhead">QPM</td>
              <td className="colhead" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="rowfollow">
                  <b>{r.name}</b>
                  <span className="ml-1 text-sub">
                    {r.revoked_at
                      ? `（${t.revoked}）`
                      : r.last_used_at
                        ? ""
                        : `（${t.neverUsed}）`}
                  </span>
                </td>
                <td className="rowfollow num">{r.rate_per_min}</td>
                <td className="rowfollow">
                  {!r.revoked_at && (
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => revoke(r.id)}
                      className="min-h-[28px] rounded-full border border-line px-3 text-[11px] font-bold text-danger disabled:opacity-50"
                    >
                      {t.revoke}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        </div>
      )}
    </div>
  );
}
