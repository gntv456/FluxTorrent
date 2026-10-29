"use client";

import { BTN_SM_SKY } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { fmt } from "@/i18n/config";

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
  // 有效/已吊销分开渲染：后端只限「最多 3 枚有效令牌」，历史吊销会无限
  // 累积，早年全部平铺时实测单账号 139 行、5275px 高，把整个安全面板顶爆。
  const activeRows = rows.filter((r) => !r.revoked_at);
  const revokedRows = rows.filter((r) => r.revoked_at);
  const [name, setName] = useState("");
  const [plain, setPlain] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 后端认三种 scope（read 恒含，upload / crossseed 需显式给）。此前前端
  // 只提交 name+password，服务端按默认值落 `read`——于是「发种」「辅种」两类
  // 客户端在界面上永远配不出来，功能只在 API 层存在（2026-10-27 实测：
  // 现网 287 枚令牌全是 {read}，{read,upload} 只有 1 枚、{read,crossseed}
  // 10 枚全是被撤的闸门夹具）。
  const [scopeUpload, setScopeUpload] = useState(false);
  const [scopeCross, setScopeCross] = useState(false);
  // P-113：Token 是 180 天的长期凭证，改密也不会让它失效，服务端现在要
  // 复核凭证才签。启用态探测沿用 twofa-setup / passkeys-card 的同一口径
  // （/me/overview 的 totp_enabled），服务端仍会自己再核一遍。
  const [pw, setPw] = useState("");
  const [code, setCode] = useState("");
  // P-608：null = 状态未知（/me/overview 探测失败）。不能当成「没开 2FA」：
  // 开了的账号会连验证码输入框都不画，复核被服务端驳回时无处可填。
  const [twofa, setTwofa] = useState<boolean | null>(null);

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
      .catch(() => setTwofa(null));
  }, [load]);

  async function issue() {
    if (!name.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ token: string }>("/api/v1/me/tokens", {
        name: name.trim(),
        password: pw,
        scopes: [
          "read",
          ...(scopeUpload ? ["upload"] : []),
          ...(scopeCross ? ["crossseed"] : []),
        ],
        ...(code.length === 6 ? { totp: Number(code) } : {}),
      });
      setPlain(r.token);
      setName("");
      setPw("");
      setCode("");
      setScopeUpload(false);
      setScopeCross(false);
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
        {twofa !== false && (
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
        <label className="flex items-center gap-2 text-xs text-sub">
          <input
            type="checkbox"
            checked={scopeUpload}
            onChange={(e) => setScopeUpload(e.target.checked)}
          />
          {t.scopeUpload}
        </label>
        <label className="flex items-center gap-2 text-xs text-sub">
          <input
            type="checkbox"
            checked={scopeCross}
            onChange={(e) => setScopeCross(e.target.checked)}
          />
          {t.scopeCrossseed}
        </label>
        <button
          type="button"
          disabled={
            busy || !name.trim() || !pw || (twofa === true && code.length < 6)
          }
          onClick={issue}
          className={BTN_SM_SKY}
        >
          {t.issue}
        </button>
      </div>
      <p className="text-xs text-sub">{t.scopeHint}</p>

      {plain && (
        <p
          className="rounded-[var(--r-md)] bg-[var(--tone-warn-soft)]
            p-2 text-xs"
        >
          {t.plainOnce}
          <code className="num ml-1 break-all font-bold">{plain}</code>
        </p>
      )}
      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}

      {activeRows.length > 0 && (
        <div className="brand-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{t.title}</td>
              <td className="colhead">QPM</td>
              <td className="colhead" />
            </tr>
          </thead>
          <tbody>
            {activeRows.map((r) => (
              <tr key={r.id}>
                <td className="rowfollow">
                  <b>{r.name}</b>
                  <span className="ml-1 text-sub">
                    {r.last_used_at ? "" : `（${t.neverUsed}）`}
                  </span>
                </td>
                <td className="rowfollow num">{r.rate_per_min}</td>
                <td className="rowfollow">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => revoke(r.id)}
                    className="min-h-[28px] rounded-full border border-line px-3 text-[11px] font-bold text-danger disabled:opacity-50"
                  >
                    {t.revoke}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        </div>
      )}

      {/* 已吊销令牌默认收起：后端只限「最多 3 枚有效」，历史吊销会无限累积，
          早年无脑平铺时实测单账号 139 行、5275px 高，把整个安全面板顶爆。
          需要追溯时再展开。 */}
      {revokedRows.length > 0 && (
        <details className="text-xs">
          <summary className="cursor-pointer select-none text-sub">
            {fmt(t.revokedHistory, { n: revokedRows.length })}
          </summary>
          <div className="brand-wide-table-scroll mt-1">
            <table className="nexus-table text-xs">
              <tbody>
                {revokedRows.map((r) => (
                  <tr key={r.id}>
                    <td className="rowfollow">
                      <s className="text-sub">{r.name}</s>
                      <span className="ml-1 text-sub">（{t.revoked}）</span>
                    </td>
                    <td className="rowfollow num">{r.rate_per_min}</td>
                    <td className="rowfollow" />
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </details>
      )}
    </div>
  );
}
