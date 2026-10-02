"use client";

import { PANEL_LG } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useTenDraw } from "@/lib/ten-draw";
import {
  SwCtaRow,
  SwHist,
  SwStatRow,
  SwTenModal,
} from "@/components/game/sw-panels";
import { SwPrizeStrip } from "@/components/game/sw-enrich";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { ResultFlash } from "@/components/game/game-kit-feedback";
import { StreakPill } from "@/components/game/streak-pill";
import {
  CapsuleMachine,
  type CapsulePrize,
} from "@/components/game/capsule-machine";
import { GameStage } from "@/components/game/game-stage";

export interface Overview {
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
  capsule?: { ticket: number; prizes: CapsulePrize[] };
}

interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
  at?: string;
}

interface DrawResult {
  index: number;
  prize: string;
  payout: number;
  net: number;
  kind?: "magic" | "item" | "fallback";
  value?: number;
  fell_back?: string | null;
}

/** 扭蛋机专注页：投币 → 震机 → 掉蛋 → 开壳揭晓。 */
export default function CapsuleFocusPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tc = dict.games.capsule;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [hist, setHist] = useState<RoundRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<DrawResult | null>(null);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // 样图手机版：主玩法 / 记录图鉴 双 tab
  const [tab, setTab] = useState("play");
  const [reduced, setReduced] = useState(false);
  const [sessionPlays, setSessionPlays] = useState(0);
  const [streak, setStreak] = useState(0);
  // 个人收集（趣味性批）：本人真实抽中数，替代纯概率的收集感。
  // 数据来自通用图鉴端点 /games/collection/capsule（与 jgg/scratch/wheel 同源）
  const [collect, setCollect] = useState<{
    tiers: { tier: string; got: number; total: number; hits: number }[];
  } | null>(null);
  const idem = useRef<string>("");

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  const loadCollect = useCallback(async () => {
    try {
      const r = await api.get<{
        entries: { rarity: number; count: number }[];
      }>("/api/v1/games/collection/capsule");
      type Bucket = { got: number; total: number; hits: number };
      const byRar: Record<number, Bucket> = {};
      for (const e of r.entries) {
        const b = (byRar[e.rarity] ??= { got: 0, total: 0, hits: 0 });
        b.total += 1;
        if (e.count > 0) {
          b.got += 1;
          b.hits += e.count;
        }
      }
      const name = (rr: number) => (rr >= 4 ? "SSR" : rr === 3 ? "SR" : "R");
      setCollect({
        tiers: [4, 3, 2]
          .filter((rr) => byRar[rr]?.total)
          .map((rr) => ({ tier: name(rr), ...byRar[rr] })),
      });
    } catch {
      /* 未登录或失败：收集块整块隐藏，不影响玩法 */
    }
  }, []);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* ignore */
    }
    try {
      setHist(
        await api.get<RoundRow[]>("/api/v1/games/rounds?game=capsule&limit=10"),
      );
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
    void loadCollect();
  }, [loadMeta, loadCollect]);

  const prizes = ov?.capsule?.prizes ?? [];
  const ticket = ov?.capsule?.ticket ?? 100;

  // R-SR-SSR 收集度（样图⑦）：按奖池稀有度权重分布实时合成（公示口径）
  const rarityMeters = (() => {
    const buckets: Record<number, number> = {};
    for (const p of prizes) {
      const r = (p as { rarity?: number }).rarity ?? 1;
      buckets[r] = (buckets[r] ?? 0) + p.weight_permille;
    }
    const total = Object.values(buckets).reduce((a, b) => a + b, 0) || 1;
    return [2, 3, 4].map((r) => ({
      r,
      pct: Math.round(((buckets[r] ?? 0) / total) * 100),
    }));
  })();
  // 奖池条（样图⑦ 扭蛋屏）：等值前 4 档（CapsulePrize 无 rarity 字段，
  //  按「等值/票价」分色调：≥10x 金、≥3x 紫、其余蓝）
  const prizeStripItems = prizes
    .slice()
    .sort(
      (a, b) => (b.value ?? b.payout * ticket) - (a.value ?? a.payout * ticket),
    )
    .slice(0, 4)
    .map((p) => {
      const v = p.value ?? p.payout * ticket;
      return {
        ic: p.kind === "item" ? (p.icon ?? "🎁") : "💎",
        nm: p.label,
        odds: `${(p.weight_permille / 10).toFixed(1)}%`,
        tone:
          v >= ticket * 10
            ? ("gold" as const)
            : v >= ticket * 3
              ? ("lilac" as const)
              : ("sky" as const),
      };
    });
  const ten = useTenDraw<DrawResult>({
    path: "/api/v1/games/capsule",
    onError: (msg) => setErr(msg),
    onDone: () => {
      setSessionPlays((n) => n + 10);
      void loadMeta();
      void loadCollect();
    },
    netOf: (r) => (r.value ?? r.payout) - ticket,
    labelOf: (r) => r.prize,
  });
  const winRate = prizes.length
    ? (100 - (prizes[0]?.weight_permille ?? 0) / 10).toFixed(1)
    : "—";

  async function draw() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    setFlash(null);
    setRes(null);
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID
        ? crypto.randomUUID()
        : String(Date.now());
    try {
      const r = await api.post<DrawResult>("/api/v1/games/capsule", {
        idempotency_key: idem.current,
      });
      setRes(r);
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
      setBusy(false);
    }
  }

  function onLanded() {
    const r = res;
    setBusy(false);
    setSessionPlays((n) => n + 1);
    if (!r) return;
    // 物品位 payout 为 0：必须按 value 判，否则「中了免考核卡」会渲染成谢谢参与
    const won = r.value ?? r.payout;
    const kind: "win" | "lose" | "tie" | "jackpot" =
      won >= ticket * 10
        ? "jackpot"
        : won > ticket
          ? "win"
          : won === ticket
            ? "tie"
            : "lose";
    setFlash({
      kind,
      text:
        won >= ticket * 10
          ? fmtCur(tc.jackpot, { prize: r.prize, net: won - ticket }, currency)
          : won > ticket
            ? fmtCur(tc.win, { prize: r.prize, net: won - ticket }, currency)
            : won === ticket
              ? tc.again
              : tc.thanks,
    });
    // 连胜（趣味性批）：>票价续 1、否则清零
    setStreak((st) => (won > ticket ? st + 1 : 0));
    void loadMeta();
    void loadCollect();
  }

  if (tab === "records") {
    return (
      <div className="flex flex-col gap-4">
        <GameTabs
          tabs={[
            { key: "play", label: dict.games.sub.tabPlay, icon: "🎲" },
            { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
          ]}
          active={tab}
          onChange={setTab}
        />
        <GameRecordsPanel game="capsule" />
      </div>
    );
  }
  return (
    <GameShell
      icon={<ArcadeGlyph k="capsule" />}
      title={tc.title}
      subtitle={`${tc.winRate.replace("{pct}", winRate)} · ${tc.sub}`}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      notice={
        <>
          <GameTabs
            tabs={[
              { key: "play", label: dict.games.sub.tabPlay, icon: "🎲" },
              { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
            ]}
            active={tab}
            onChange={setTab}
          />
          <PlayHint
            sessionPlays={sessionPlays}
            todayNet={ov?.me?.today_net ?? null}
          />
        </>
      }
      stage={
        <div className="flex w-full flex-col items-center gap-3">
          <GameStage
            bg="radial-gradient(120% 130% at 50% 0%, #3a2a5e, #14101f 72%)"
            className="w-full max-w-[460px] py-6"
          >
            <CapsuleMachine
              prizes={prizes}
              ticket={ticket}
              resultIndex={res?.index ?? null}
              busy={busy}
              disabled={prizes.length === 0}
              reduced={reduced}
              onDraw={draw}
              onLanded={onLanded}
              goLabel={tc.go}
              drawingLabel={tc.drawing}
            />
          </GameStage>
          <ResultFlash
            kind={flash?.kind ?? null}
            text={flash?.text ?? (busy ? tc.drawing : null)}
          />
          <StreakPill n={streak} label={t.streakLabel} />
          <SwStatRow
            items={[
              { lb: tc.statPlays, vl: String(ov?.me?.today_plays ?? 0) },
              { lb: tc.statTicket, vl: String(ticket), tone: "gold" },
              {
                lb: tc.statLeft,
                vl: ov?.me?.limit_left != null ? String(ov.me.limit_left) : "—",
                tone: "green",
              },
            ]}
          />
          <SwPrizeStrip label={tc.prizeLb} items={prizeStripItems} />
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className={PANEL_LG}>
          <h2 className="mb-2 font-display text-base">{tc.prizePool}</h2>
          <div className="flex flex-wrap gap-1.5">
            {prizes.map((p, i) => (
              <span
                key={i}
                className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
                  (p.value ?? p.payout * ticket) >= ticket * 10
                    ? "bg-sun-soft text-[var(--warning)]"
                    : (p.value ?? p.payout * ticket) > ticket
                      ? "bg-mint-soft text-[var(--mint)]"
                      : "bg-[var(--surface-sunken)] text-sub"
                }`}
              >
                {p.label} {(p.weight_permille / 10).toFixed(1)}%
              </span>
            ))}
          </div>
          <p className="mt-2 text-[11px] text-sub">{tc.poolNote}</p>
          <div className="sw-cap-meters">
            {rarityMeters.map((m) => (
              <div key={m.r} className="sw-cap-meter">
                <span className="sw-cap-r num">
                  {m.r >= 4 ? "SSR" : m.r === 3 ? "SR" : "R"}
                </span>
                <span className="sw-cap-track">
                  <i style={{ width: `${m.pct}%` }} />
                </span>
                <span className="num sw-cap-pct">{m.pct}%</span>
              </div>
            ))}
          </div>
          {/* 个人收集（趣味性批）：本人真实抽中数，替代纯概率的收集感。
              got/total = 集齐档位数；hits = 累计抽中次数 */}
          {collect && (
            <div className="sw-cap-collect">
              {collect.tiers.map((tr) => (
                <div key={tr.tier} className="sw-cap-crow">
                  <b className="num sw-cap-ctier">{tr.tier}</b>
                  <span className="num">
                    {tc.collectGot
                      .replace("{a}", String(tr.got))
                      .replace("{b}", String(tr.total))}
                  </span>
                  {tr.hits > 0 && (
                    <span className="num sw-cap-chits">×{tr.hits}</span>
                  )}
                </div>
              ))}
            </div>
          )}
          <SwCtaRow
            primaryLabel={`${tc.go} · ${ticket}`}
            goldLabel={`${tc.tenBtn} · ${ticket * 10}`}
            onPrimary={draw}
            onGold={() => void ten.run()}
            primaryDisabled={busy || prizes.length === 0}
            goldDisabled={ten.busy || busy || prizes.length === 0}
            goldBusy={ten.busy}
          />
        </div>
      }
      side={
        <div className={PANEL_LG}>
          <h2 className="mb-2 font-display text-base">{t.history}</h2>
          <SwHist
            rows={hist.slice(0, 6).map((h) => ({
              t: (h.at ?? "").slice(11, 16) || "—",
              txt: h.net > 0 ? tc.histWin : tc.histLose,
              net: h.net,
            }))}
            empty={t.historyEmpty}
          />
        </div>
      }
      foot={
        <SwTenModal
          open={ten.open}
          title={tc.tenTitle}
          rows={ten.rows}
          totalLabel={tc.tenTotal.replace("{magic}", currency)}
          onClose={ten.close}
        />
      }
    />
  );
}
