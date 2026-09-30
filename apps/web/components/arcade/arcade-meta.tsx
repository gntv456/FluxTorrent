"use client";

/**
 * 娱乐屋「大厅游戏表面」面板（数据由 RSC 预取后传入，SSR 直出首屏）。
 * 五件器件：纹章预算环 / 周常任务条 / 票根册 / 赛季星轨 / 外观货架 + 门禁自检。
 * 领取按钮走 POST → router.refresh() 回读真值（乐观更新留给后续优化）。
 */

import { useState, type CSSProperties } from "react";
import { useRouter } from "next/navigation";
import { useI18n } from "@/i18n/client";
import { api } from "@/lib/api-client";
import { ArcadeFeed, type ArcadeFeedItem } from "./arcade-feed";
import { ArcadeAlbum } from "./arcade-album";
import { ArcadeBackpack, type ArcadePackData } from "./arcade-backpack";
import { ArcadeBoard, type ArcadeBoardData } from "./arcade-board";

export interface ArcadeLedger {
  magic_back: number;
  det_cost: number;
  net: number;
  budget: number;
  base: number;
  pct: number;
  window_days: number;
}
export interface ArcadeQuest {
  code: string;
  done: number;
  target: number;
  claimed: boolean;
  ready: boolean;
  reward: number;
  /** 0247 起确定侧奖励可以发物品：件数与目录名一起下发，前台不再猜 */
  item_name?: string | null;
  item_qty?: number;
}
export interface ArcadeStub {
  code: string;
  name: string;
  descr: string;
  obtained: boolean;
  holders: number;
}
export interface ArcadeMilestone {
  code: string;
  need: number;
  reward: number;
  claimed: boolean;
  reached: boolean;
  ready: boolean;
  item_name?: string | null;
  item_qty?: number;
}
export interface ArcadeShelfItem {
  id: number;
  name: string;
  kind: string;
  price: number;
  owned: boolean;
  zero_debt: boolean;
}
export interface ArcadeCheck {
  name: string;
  pass: boolean | null;
  why: string;
}
export interface ArcadeMeta {
  ledger: ArcadeLedger;
  quests: { period: string; items: ArcadeQuest[] };
  stubs: { owned: number; total: number; items: ArcadeStub[] };
  season: { key: string; items: ArcadeMilestone[] };
  shelf: ArcadeShelfItem[];
  backpack: ArcadePackData;
  board: ArcadeBoardData;
  feed: ArcadeFeedItem[];
  checks: ArcadeCheck[];
}

const fmt = (n: number) => Math.round(n).toLocaleString("en-US");

export function ArcadeMeta({ initial }: { initial: ArcadeMeta }) {
  const { dict, currency } = useI18n();
  const t = dict.games.arcade;
  const router = useRouter();
  const [busy, setBusy] = useState<string | null>(null);

  const L = initial.ledger;
  const p = L.budget > 0 ? Math.min(1, L.det_cost / L.budget) : 0;
  const red = initial.checks.filter((c) => c.pass === false);
  const qText: Record<string, string> = {
    q_scratch: t.q_scratch,
    q_bs: t.q_bs,
    q_farm: t.q_farm,
    q_week: t.q_week,
  };
  const mText: Record<string, string> = {
    m1: t.m1,
    m2: t.m2,
    m3: t.m3,
    m4: t.m4,
  };

  async function claim(kind: "quest" | "season", code: string) {
    const key = kind + ":" + code;
    setBusy(key);
    try {
      await api.post(
        `/api/v1/games/arcade/${kind}/${code}/claim`,
        {},
      );
      router.refresh();
    } catch {
      // 失败静默：下一轮 refresh 会回读真实态
    } finally {
      setBusy(null);
    }
  }

  // 奖励文案：魔力与物品都要说得出名字。确定侧现在能发物品了，
  // 前台只报数字的话，玩家不知道自己领到的是券还是装扮。
  const award = (n: number, item?: string | null, qty?: number) => {
    const parts: string[] = [];
    if (n > 0) parts.push(t.rewardSpark.replace("{n}", fmt(n)));
    if (item) parts.push(`+${qty ?? 1} ${item}`);
    return parts.join(" · ");
  };

  return (
    <div className="arc">
      <div className="arc-hd">
        <h2>{t.title}</h2>
        <span className={`arc-gate ${red.length ? "warn" : "ok"}`}>
          {red.length
            ? t.gateBlocked.replace("{n}", String(red.length))
            : t.gateOk}
        </span>
      </div>

      {/* 纹章面板：确定侧发放预算环（EV 闸只管随机侧，这是第二道闸） */}
      <section className="arc-crest crest-frame">
        <span className="arc-ribbon">
          {t.crestRibbon
            .replace("{s}", initial.season.key)
            .replace("{n}", String(initial.stubs.owned))
            .replace("{m}", String(initial.stubs.total))}
        </span>
        <div className="arc-crest-side">
          <div className="arc-bignum">
            {L.net >= 0 ? "+" : ""}
            {fmt(L.net)}
          </div>
          <div className="arc-qsub">{t.crestNet}</div>
        </div>
        <div
          className="arc-ring"
          style={{ "--p": p.toFixed(3) } as CSSProperties}
          title={`${(p * 100).toFixed(1)}%`}
        >
          <b>{(p * 100).toFixed(1)}%</b>
        </div>
        <div className="arc-crest-side r">
          <div className="arc-bignum">
            {fmt(L.det_cost)}
            <small> / {fmt(L.budget)}</small>
          </div>
          <div className="arc-qsub">{t.crestBudget}</div>
        </div>
      </section>
      <p className="arc-note">{t.crestNote.replace("{magic}", currency)}</p>

      {/* 周常任务条 */}
      <section className="arc-sec">
        <h3>
          {t.questsTitle} <span className="num">{initial.quests.period}</span>
        </h3>
        {initial.quests.items.map((q) => (
          <div key={q.code} className={`arc-q${q.ready ? " ready" : ""}`}>
            <div>
              <div>{qText[q.code] ?? q.code}</div>
              <div className="arc-qsub">
                {award(q.reward, q.item_name, q.item_qty)}
              </div>
              <div className="bar">
                <i
                  style={{
                    width: `${Math.min(100, (q.done / q.target) * 100)}%`,
                  }}
                />
              </div>
            </div>
            <span className="val">
              {q.done}/{q.target}
            </span>
            <button
              className={`arc-btn${q.ready ? " primary" : ""}`}
              disabled={!q.ready || busy === "quest:" + q.code}
              onClick={() => claim("quest", q.code)}
            >
              {q.claimed ? t.claimed : t.claim}
            </button>
          </div>
        ))}
      </section>

      {/* 票根册（拆给 arcade-album：这一件有自己的材质与「未获得怎么画」的口径）*/}
      <ArcadeAlbum stubs={initial.stubs} />

      {/* 赛季星轨 */}
      <section className="arc-sec">
        <h3>
          {t.trackTitle} <span className="num">{initial.season.key}</span>
        </h3>
        <div className="arc-track">
          {initial.season.items.map((m, i) => (
            <div key={m.code} className="arc-track-cell">
              {i > 0 && <span className="arc-link" />}
              <button
                className={
                  "arc-node " +
                  (m.claimed
                    ? "reached"
                    : m.ready
                      ? "ready"
                      : m.reached
                        ? "reached"
                        : "lock")
                }
                disabled={!m.ready || busy === "season:" + m.code}
                onClick={() => claim("season", m.code)}
                title={`${mText[m.code] ?? m.code} · ${award(
                  m.reward,
                  m.item_name,
                  m.item_qty,
                )}`}
              >
                {m.claimed ? "✓" : m.ready ? "★" : m.reached ? "★" : "🔒"}
              </button>
              <div className="arc-lab">
                {mText[m.code] ?? m.code} · {m.need}
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* 我的背包：奖池发出的物品要回到玩家眼前，否则「发奖」只是账面上的一行 */}
      <ArcadeBoard board={initial.board} />

      <ArcadeBackpack pack={initial.backpack} />

      {/* 外观货架 */}
      <section className="arc-sec">
        <h3>{t.shelfTitle}</h3>
        <div className="arc-shelf">
          {initial.shelf.map((g) => (
            <div key={g.id} className="arc-goods">
              {g.zero_debt && <span className="zero">{t.zeroDebt}</span>}
              <div className="gn">{g.name}</div>
              <div className="gv">
                {fmt(g.price)} {currency}
                {g.owned ? ` · ${t.owned}` : ""}
              </div>
            </div>
          ))}
          {initial.shelf.length === 0 && (
            <div className="arc-note">{t.shelfEmpty}</div>
          )}
        </div>
      </section>

      {/* 全服公示：谁点亮了票根 / 领了奖励（社交钩子） */}
      <ArcadeFeed items={initial.feed} />

      {/* 门禁自检 */}
      <section className="arc-sec">
        <h3>{t.gateTitle}</h3>
        <div className="arc-checks">
          {initial.checks.map((c) => (
            <div key={c.name} className="arc-chk">
              <span className={`dot${c.pass === false ? " r" : ""}`} />
              <span>
                {c.name}
                {c.why && <span className="why"> · {c.why}</span>}
              </span>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
