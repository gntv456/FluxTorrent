"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmtCur } from "@/i18n/config";
import { BTN_SM_BOLD, BTN_SM_GHOST } from "@/lib/ui-classes";
import { StatePanel } from "@/components/ui/state-panel";
import { SkeletonRows } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/ui/page-header";
import { Pager } from "@/components/ui/pager";

interface SparkInfo {
  balance: number;
  seeding_count: number;
  hourly_estimate: number;
  /** 收益构成：[规则名, 颗数]（worker 同源口径） */
  reward_rules?: [string, number][];
}

/** 与后端 /me/spark/ledger 的 per 固定值一致（50/页） */
const LEDGER_PER = 50;

interface LedgerEnvelope {
  items: LedgerRow[];
  total: number;
  page: number;
  per: number;
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
  // 2026-10-16 改真分页：此前是 limit 20..50 的「加载更多」，封顶 50 条后
  // 再也看不到更早的流水，且界面没有任何「还有更多」的提示。后端补 page/total。
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  // 样图 § my-spark 的「兑换邀请（15,000）」：价格取 /invites/status 的
  // redeem_price 真值（invite_http.rs:119），取不到就只显示按钮名
  const [redeemPrice, setRedeemPrice] = useState<number | null>(null);
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
      .get<{ redeem_price?: number | null }>(
        "/api/v1/invites/status",
      )
      .then((d) => setRedeemPrice(d.redeem_price ?? null))
      .catch(() => setRedeemPrice(null));
    api
      .get<LedgerEnvelope>(`/api/v1/me/spark/ledger?page=${page}`)
      .then((d) => {
        setRows(d.items);
        setTotal(d.total);
      })
      .catch((e) => {
        setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
        setRows([]);
      });
  }, [page, dict]);
  useEffect(load, [load]);

  if (!mounted) {
    // 与 SSR 一致的中性态（必须在所有 hooks 之后，否则破坏 hooks 顺序）
    return (
      <div className="flex flex-col gap-4">
        <h1 className="font-display text-2xl">
          {t.title.replace("{magic}", currency)}
        </h1>
        {/* A3 四态：裸「加载中」文字 → 骨架占位 */}
        <SkeletonRows rows={5} />
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
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={t.title.replace("{magic}", currency)}
        sub={t.subtitle}
        aside={
          <>
            <span className="pill">
              {t.seedingCount} {info ? info.seeding_count : "—"}
            </span>
            <span className="pill">
              {t.hourly} {info ? `+${info.hourly_estimate}` : "—"}
            </span>
          </>
        }
      />

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

      {/* 四个去向都是站内真路由：/donate 充值、/invites 兑换邀请名额、
          /shop 道具、/magic-pool 魔力池。样图 § my-spark 有这一排，
          此前整块缺失，用户看完余额不知道能干什么。 */}
      <div className="flex flex-wrap gap-2">
        <Link href="/donate" className={BTN_SM_BOLD}>
          {fmtCur(t.ctaRecharge, {}, currency)}
        </Link>
        <Link href="/invites" className={BTN_SM_GHOST}>
          {t.ctaRedeem}
          {redeemPrice !== null
            ? `（${redeemPrice.toLocaleString()}）`
            : ""}
        </Link>
        <Link href="/shop" className={BTN_SM_GHOST}>
          {t.ctaShop}
        </Link>
        <Link href="/magic-pool" className={BTN_SM_GHOST}>
          {t.ctaPool}
        </Link>
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
         *  客户端组件但免 useIsCompact 双树——流水 4 列窄屏改行读。
         *  A3 四态：流水表 loading 骨架 / 错误占位 / 空态走 StatePanel */}
        <StatePanel
          loading={rows.length === 0 && err === null}
          error={rows.length === 0 ? err : null}
          empty={rows.length === 0}
          loadingFallback={<SkeletonRows rows={5} />}
          onRetry={load}
        >
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
            </tbody>
          </table>
        </div>
        </StatePanel>
      </div>

      {total > LEDGER_PER && (
        <Pager
          layout="compact"
          page={page}
          pages={Math.ceil(total / LEDGER_PER)}
          posLabel={`${page} / ${Math.ceil(total / LEDGER_PER)}`}
          onStep={setPage}
        />
      )}
    </div>
  );
}
