"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { BTN_SM_BOLD, BTN_SM_GHOST } from "@/lib/ui-classes";
import { SkeletonRows } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/ui/page-header";
import { TabPanel, Tabs } from "@/components/ui/tabs";
import { MerklePanel } from "./_parts/merkle-panel";
import { SeedingCalc } from "./_parts/seeding-calc";
import { ExcludedNote } from "./_parts/excluded-note";
import { LedgerPanel } from "./_parts/ledger-panel";
import {
  LEDGER_PER,
  sums,
  type LedgerEnvelope,
  type LedgerRow,
  type Summary,
} from "./_parts/ledger-types";
import type { SparkInfo } from "./_parts/spark-types";

/** 我的魔力（mybonus.php 口径）：余额卡片 + 收益估算 + 最近流水。
 *  2026-11-13 标签化（省高改版续）：统计条/CTA/排除提示常驻，正文三域
 *  收进标签组（流水默认 / 收益计算 / 每日凭证）——复用 ui/tabs 回调档
 *  + 挂载门卫（激活过的 tab 保持挂载，未访问零渲染零请求：Merkle 挂载
 *  即拉 manifest，SeedingCalc 大表渲染也可观）。 */
export default function MySparkPage() {
  const { dict, currency } = useI18n();
  const t = dict.myspark;
  const [info, setInfo] = useState<SparkInfo | null>(null);
  const [rows, setRows] = useState<LedgerRow[]>([]);
  // 2026-10-16 改真分页：旧「加载更多」封顶 50 条且无任何「还有更多」提示
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  // 近期收支固定取第一页（最近 50 笔），翻页只换表格不换合计
  const [summary, setSummary] = useState<Summary | null>(null);
  // 样图 § my-spark 的「兑换邀请（15,000）」：价格取 /invites/status 的
  // redeem_price 真值（invite_http.rs:119），取不到就只显示按钮名
  const [redeemPrice, setRedeemPrice] = useState<number | null>(null);
  // 标签组：tab 状态 + 已访问集合（挂载门卫，语义同详情页标签组）
  const [tab, setTab] = useState("ledger");
  const [visited, setVisited] = useState<ReadonlySet<string>>(
    () => new Set(["ledger"]),
  );
  const switchTab = (k: string) => {
    setTab(k);
    setVisited((prev) => (prev.has(k) ? prev : new Set(prev).add(k)));
  };
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
        if (page === 1) setSummary(sums(d.items));
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
        <p className="brand-panel p-4 text-sm text-sub">
          {dict.common.pleaseLogin}
        </p>
      </div>
    );
  }

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
              {info?.earning_seeding_count !== undefined &&
                info.earning_seeding_count < info.seeding_count && (
                  <>
                    {" · "}
                    {t.earningSplit} {info.earning_seeding_count}
                  </>
                )}
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
          <div className="v">
            {summary?.income !== undefined
              ? summary.income.toLocaleString()
              : "—"}
          </div>
        </div>
        <div>
          <div className="k">{t.recentSpend}</div>
          <div className="v">
            {summary?.spend !== undefined
              ? summary.spend.toLocaleString()
              : "—"}
          </div>
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
      {/* 未计收益要给原因和可行动的修法，见 excluded-note.tsx
          （行动告警不进 tab：任何 tab 下都要看得见） */}
      {info && <ExcludedNote info={info} t={t} />}

      {/* 正文三域标签化：流水（默认）/ 收益计算 / 每日凭证。
          挂载门卫：激活过才挂载、切回不重拉；方向键/roving tabindex
          由 ui/tabs 原语承担 */}
      <Tabs
        idPrefix="ms"
        label={t.title.replace("{magic}", currency)}
        value={tab}
        onChange={switchTab}
        items={[
          { key: "ledger", label: t.ledger.replace("{magic}", currency) },
          { key: "calc", label: t.calcTitle },
          { key: "merkle", label: t.merkleTitle },
        ]}
      />

      <TabPanel idPrefix="ms" tabKey="ledger">
        <LedgerPanel
          rows={rows}
          total={total}
          page={page}
          setPage={setPage}
          summary={summary}
          err={err}
          onRetry={load}
        />
      </TabPanel>

      {/* 收益计算：info 就绪才可看（无 tiers 时组件自隐，面板留空）。
          id 对齐 ui/tabs 的 aria-controls（panel-ms-calc/merkle） */}
      <div id="panel-ms-calc" role="tabpanel" hidden={tab !== "calc"}>
        {(tab === "calc" || visited.has("calc")) && info && (
          <SeedingCalc info={info} t={t} currency={currency} />
        )}
      </div>

      {/* 每日凭证：挂载即拉 manifest——激活才挂载 */}
      <div id="panel-ms-merkle" role="tabpanel" hidden={tab !== "merkle"}>
        {(tab === "merkle" || visited.has("merkle")) && <MerklePanel />}
      </div>
    </div>
  );
}
