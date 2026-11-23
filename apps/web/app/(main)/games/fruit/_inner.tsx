"use client";
import {
  BTN_RETRY_PILL,
  PANEL_LG,
  PANEL_WARN_ORANGE,
  STAGE_STAT_LINE,
} from "@/lib/ui-classes";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError, isRetryableNetError } from "@/lib/api-client";
import { useTenDraw } from "@/lib/ten-draw";
import { CapsulePoolTab } from "@/components/game/capsule-pool-tab";
import { SideTabs } from "@/components/game/side-tabs";
import { useRounds } from "@/components/game/use-rounds";
import { SwCtaRow, SwHist, SwTenModal } from "@/components/game/sw-panels";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { ResultFlash } from "@/components/game/game-kit-feedback";
import { StreakPill } from "@/components/game/streak-pill";
import {
  FruitMachine,
} from "@/components/game/fruit-machine";
import type { SlotPrize } from "@/components/game/slot-machine";
import { GameStage } from "@/components/game/game-stage";
import { clientUuid } from "@/lib/client-uuid";

export interface Overview {
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
  fruit?: { ticket: number; prizes: SlotPrize[] };
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

/** 水果机专注页：投币 → 灯排跑马 → 中央窗落符号揭晓。 */
export default function FruitFocusPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tc = dict.games.fruit;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const { hist, streak, reload: reloadRounds } = useRounds("fruit");
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<DrawResult | null>(null);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [tab, setTab] = useState("play");
  const [reduced, setReduced] = useState(false);
  const [sessionPlays, setSessionPlays] = useState(0);
  const [collect, setCollect] = useState<{
    tiers: { tier: string; got: number; total: number; hits: number }[];
  } | null>(null);
  const idem = useRef<string>("");
  const [unknown, setUnknown] = useState(false);

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  const loadCollect = useCallback(async () => {
    try {
      const r = await api.get<{
        entries: { rarity: number; count: number }[];
      }>("/api/v1/games/collection/fruit");
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
    void reloadRounds();
  }, [reloadRounds]);

  useEffect(() => {
    void loadMeta();
    void loadCollect();
  }, [loadMeta, loadCollect]);

  const prizes = ov?.fruit?.prizes ?? [];
  const ticket = ov?.fruit?.ticket ?? 100;

  const ten = useTenDraw<DrawResult>({
    path: "/api/v1/games/fruit",
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

  async function draw(reuse = false) {
    if (busy) return;
    setBusy(true);
    setErr(null);
    setFlash(null);
    setRes(null);
    if (!idem.current) idem.current = clientUuid();
    setUnknown(false);
    try {
      const r = await api.post<DrawResult>("/api/v1/games/fruit", {
        idempotency_key: idem.current,
      });
      setRes(r);
      idem.current = "";
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (apiErrorMessage(dict, e))
          : dict.common.networkError,
      );
      if (isRetryableNetError(e)) {
        setUnknown(true);
      }
      setBusy(false);
    }
  }

  function onLanded() {
    const r = res;
    setBusy(false);
    setSessionPlays((n) => n + 1);
    if (!r) return;
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
        <GameRecordsPanel game="fruit" />
      </div>
    );
  }
  return (
    <GameShell
      icon={<ArcadeGlyph k="fruit" />}
      title={tc.title}
      subtitle={`${tc.winRate.replace("{pct}", winRate)} · ${tc.sub}`}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      tabBar={
        <GameTabs
          tabs={[
            { key: "play", label: dict.games.sub.tabPlay, icon: "🎲" },
            { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
          ]}
          active={tab}
          onChange={setTab}
        />
      }
      notice={
        <PlayHint
          sessionPlays={sessionPlays}
          todayNet={ov?.me?.today_net ?? null}
        />
      }
      stage={
        <div className="flex w-full flex-col items-center gap-3">
          <GameStage
            bg="radial-gradient(120% 130% at 50% 0%, #123a6e, #0a1526 72%)"
            shjBg="var(--shj-stage-fruit)"
            className="w-full max-w-[460px] py-6 relative jgg-scene-host"
          >
            <FruitMachine
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
              title={tc.title}
            />
          </GameStage>
          <ResultFlash
            kind={flash?.kind ?? null}
            text={flash?.text ?? (busy ? tc.drawing : null)}
          />
          <StreakPill n={streak} label={t.streakLabel} />
          {/* 布局优化（2026-11-23）：三格统计压成一行细条，奖池移入
              侧栏分区标签。 */}
          <div className={STAGE_STAT_LINE}>
            <span>
              {tc.statPlays}{" "}
              <b className="num text-ink">{ov?.me?.today_plays ?? 0}</b>
            </span>
            <span>
              {tc.statTicket}{" "}
              <b className="num text-[var(--warning)]">{ticket}</b>
            </span>
            <span>
              {tc.statLeft}{" "}
              <b className="num text-[var(--mint)]">
                {ov?.me?.limit_left != null ? String(ov.me.limit_left) : "—"}
              </b>
            </span>
          </div>
          {err && <p className="text-xs text-[var(--danger-text)]">{err}</p>}
          {unknown && (
            <div className={PANEL_WARN_ORANGE}>
              <p>{dict.review.resultUnknownBody}</p>
              <button
                type="button"
                className={BTN_RETRY_PILL}
                onClick={() => void draw(true)}
              >
                {dict.review.retrySameKey}
              </button>
            </div>
          )}
        </div>
      }
      controls={
        <div className={PANEL_LG}>
          <p className="text-xs text-sub">{tc.poolNote}</p>
              <SwCtaRow
                primaryLabel={`${tc.go} · ${ticket}`}
                goldLabel={`${tc.tenBtn} · ${ticket * 10}`}
                onPrimary={() => void draw()}
                onGold={() => void ten.run()}
                primaryDisabled={busy || prizes.length === 0}
                goldDisabled={ten.busy || busy || prizes.length === 0}
                goldBusy={ten.busy}
              />
        </div>
      }
      side={
        /* 布局优化（2026-11-23）：奖池 / 稀有度 / 收集进度从控制卡移入
           侧栏分区标签——控制卡只留下注按钮，一眼能玩。 */
        <SideTabs
          items={[
            {
              key: "pool",
              label: tc.prizePool,
              content: (
                <CapsulePoolTab
                  game="fruit"
                  prizes={prizes}
                  ticket={ticket}
                  collect={collect}
                  collectLabel={tc.collectGot}
                />
              ),
            },
            {
              key: "hist",
              label: t.history,
              content: (
                <SwHist
                  rows={hist.slice(0, 12).map((h) => ({
                    t: (h.at ?? "").slice(11, 16) || "—",
                    txt: h.net > 0 ? tc.histWin : tc.histLose,
                    net: h.net,
                  }))}
                  empty={t.historyEmpty}
                />
              ),
            },
          ]}
        />
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
