"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface SparkInfo {
  balance: number;
  seeding_count: number;
  hourly_estimate: number;
  /** 收益构成：[规则名, 颗数]（worker 同源口径） */
  reward_rules?: [string, number][];
}

interface LedgerRow {
  amount: number;
  kind: string;
  balance_after: number | null;
  created_at: string;
  /** 商店消费带的商品名（其它流水为 null）——kind 全是 'shop' 分不清 */
  item_name?: string | null;
}

/** 我的魔力（mybonus.php 口径）：余额卡片 + 收益估算 + 最近流水 */
export default function MySparkPage() {
  const { dict, locale, currency } = useI18n();
  const t = dict.myspark;
  const [info, setInfo] = useState<SparkInfo | null>(null);
  const [rows, setRows] = useState<LedgerRow[]>([]);
  const [limit, setLimit] = useState(20);
  const [err, setErr] = useState<string | null>(null);
  // 挂载门控：hasSessionCookie() 在**渲染期**读 document.cookie，SSR 环境下
  // document 不存在 → 恒返回 false → 服务端渲染「请先登录」而客户端渲染完整
  // 页面，两棵树不同 → React #418（hydration mismatch）。
  // 首帧（含 SSR）统一出中性态，挂载后再决定分支。
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  const loggedIn = mounted && hasSessionCookie();

  const load = useCallback(() => {
    api
      .get<SparkInfo>("/api/v1/me/spark")
      .then(setInfo)
      .catch(() => setInfo(null));
    api
      .get<LedgerRow[]>(`/api/v1/me/spark/ledger?limit=${limit}`)
      .then(setRows)
      .catch((e) => {
        setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
        setRows([]);
      });
  }, [limit, dict]);
  useEffect(load, [load]);

  if (!mounted) {
    // 与 SSR 一致的中性态（必须在所有 hooks 之后，否则破坏 hooks 顺序）
    return (
      <div className="flex flex-col gap-4">
        <h1 className="font-display text-2xl">
          {t.title.replace("{magic}", currency)}
        </h1>
        <p className="baozi-panel p-4 text-sm text-sub">
          {dict.common.loading}
        </p>
      </div>
    );
  }

  if (!loggedIn) {
    return (
      <div className="flex flex-col gap-4">
        <h1 className="font-display text-2xl">
          {t.title.replace("{magic}", currency)}
        </h1>
        <p className="baozi-panel p-4 text-sm text-sub">
          {dict.common.pleaseLogin}
        </p>
      </div>
    );
  }

  const income = rows
    .filter((r) => r.amount > 0)
    .reduce((s, r) => s + r.amount, 0);
  const spend = rows
    .filter((r) => r.amount < 0)
    .reduce((s, r) => s + r.amount, 0);

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">My Spark</div>
          <h1 className="font-display text-2xl">
            {t.title.replace("{magic}", currency)}
          </h1>
        </div>
        <span className="sub">{t.subtitle}</span>
        <div className="aside">
          <span className="pill">
            {t.seedingCount} {info ? info.seeding_count : "—"}
          </span>
          <span className="pill">
            {t.hourly} {info ? `+${info.hourly_estimate}` : "—"}
          </span>
        </div>
      </div>

      <div className="pgstats">
        <div>
          <div className="k">{t.balance}</div>
          <div className="v">
            {info ? info.balance.toLocaleString() : "…"}
          </div>
        </div>
        <div>
          <div className="k">{t.recentIncome}</div>
          <div className="v">{income.toLocaleString()}</div>
        </div>
        <div>
          <div className="k">{t.recentSpend}</div>
          <div className="v">{spend.toLocaleString()}</div>
        </div>
        <div>
          <div className="k">{t.hourly}</div>
          <div className="v">
            {info ? `+${info.hourly_estimate}` : "…"}
          </div>
        </div>
      </div>

      <div className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.ruleTitle}</h2>
        </div>
        <p className="p-4 text-xs leading-relaxed text-sub">
          {t.rule.replace("{magic}", currency)}
        </p>
      </div>

      {info?.reward_rules && info.reward_rules.length > 0 && (
        <div className="baozi-panel">
          <div className="baozi-panel__head">
            <h2>{t.rewardRules}</h2>
          </div>
          <div className="flex flex-wrap gap-1.5 p-4">
            {info.reward_rules.map(([name, cnt], i) => (
              <span key={i} className="pill">
                {name} ×{cnt}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.ledger.replace("{magic}", currency)}</h2>
          <span className="text-xs text-sub">
            {t.recentIncome}{" "}
            <b className="num text-success">+{income.toLocaleString()}</b>
            {" · "}
            {t.recentSpend}{" "}
            <b className="num text-danger">{spend.toLocaleString()}</b>
          </span>
        </div>
        {/* M6.5：compact 档 CSS 双态（.profilet 同款：thead 隐藏+块化+小标签），
         *  客户端组件但免 useIsCompact 双树——流水 4 列窄屏改行读 */}
        <div className="baozi-wide-table-scroll">
          <table className="nexus-table profilet">
            <tbody>
              <tr>
                <td className="colhead">{t.ledgerKind}</td>
                <td className="colhead">{t.ledgerAmount}</td>
                <td className="colhead">{t.ledgerBalance}</td>
                <td className="colhead">{t.ledgerAt}</td>
              </tr>
              {rows.map((r, i) => (
                <tr key={i}>
                  <td>
                    <span className="profilet__label">{t.ledgerKind}</span>
                    <span className="pill">
                      {r.item_name
                        ? r.item_name
                        : (dict.my.kinds[r.kind] ?? t.otherKind)}
                    </span>
                  </td>
                  <td
                    className={`num font-bold ${
                      r.amount >= 0 ? "text-success" : "text-danger"
                    }`}
                  >
                    <span className="profilet__label">{t.ledgerAmount}</span>
                    {r.amount >= 0 ? "+" : ""}
                    {r.amount.toLocaleString()}
                  </td>
                  <td className="num">
                    <span className="profilet__label">{t.ledgerBalance}</span>
                    {r.balance_after?.toLocaleString() ?? "—"}
                  </td>
                  <td className="text-xs text-sub">
                    <span className="profilet__label">{t.ledgerAt}</span>
                    {new Date(r.created_at).toLocaleString(
                      dateLocale(locale),
                    )}
                  </td>
                </tr>
              ))}
              {rows.length === 0 && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-sub">
                    {err ?? t.ledgerEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {rows.length >= limit && (
        <button
          className="btn btn-sm self-center"
          onClick={() => setLimit((n) => Math.min(n + 20, 50))}
        >
          {t.loadMore}
        </button>
      )}
    </div>
  );
}
