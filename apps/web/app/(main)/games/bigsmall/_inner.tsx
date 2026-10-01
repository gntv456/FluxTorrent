"use client";

import { PANEL_LG, PANEL_LG_COL } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { fmtMult } from "@/lib/games";
import { ChipSelect, GameShell, PlayHint } from "@/components/game/game-kit";
import { ResultFlash } from "@/components/game/game-kit-feedback";
import { SwHist, SwStatRow } from "@/components/game/sw-panels";
import { RunwayOdometer } from "@/components/game/runway";
import { DieFace } from "@/components/game/die-face";
import { GameStage } from "@/components/game/game-stage";
import { PropBar, type PropView } from "@/components/game/bigsmall-props";

export interface Overview {
  max_bet: number;
  bigsmall?: { win_mult: number; expected_value: number };
  props?: PropView[];
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
}

interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
  at?: string;
  /** 服务端局记录里带的点数（有则近 5 局走势直接显大/小） */
  number?: number;
}

interface GuessResult {
  number: number;
  player_win: boolean;
  payout: number;
  net: number;
  tie: boolean;
  shield_refund?: number;
  effective_mult?: number | null;
  props_applied?: { key: string; effect: string; value: number }[];
}

const SPIN_MS = 1750;

/** 猜大小专注页：跑道减速定格 + 路单 + 道具栏（道具只加权魔力，不出物品） */
export default function BigSmallPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tg = dict.games.bigsmall;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [hist, setHist] = useState<RoundRow[]>([]);
  const [bet, setBet] = useState(100);
  const [busy, setBusy] = useState(false);
  const [num, setNum] = useState<number | null>(null);
  const [res, setRes] = useState<GuessResult | null>(null);
  /** 已挂载的道具 key（同类效果至多一件） */
  const [sel, setSel] = useState<string[]>([]);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie";
    text: string;
  } | null>(null);
  const [streak, setStreak] = useState(0);
  /** 本次会话已完成的局数（防沉迷软提示用） */
  const [sessionPlays, setSessionPlays] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [reduced, setReduced] = useState(false);
  const idem = useRef<string>("");
  const timer = useRef<number | null>(null);

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* ignore */
    }
    try {
      setHist(
        await api.get<RoundRow[]>(
          "/api/v1/games/rounds?game=bigsmall&limit=20",
        ),
      );
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  useEffect(
    () => () => {
      if (timer.current) window.clearTimeout(timer.current);
    },
    [],
  );

  const maxBet = ov?.max_bet ?? 1000;
  // 赔率由后端下发（< 2.0，回收口径），不在前端写死
  const winMult = ov?.bigsmall?.win_mult ?? 1.9;
  const props = ov?.props ?? [];
  // 近 N 局统计（按局聚合的战绩算，不含流水噪音）
  const stats = hist.reduce(
    (a, r) => {
      if (r.net > 0) a.w += 1;
      else if (r.net === 0) a.t += 1;
      else a.l += 1;
      return a;
    },
    { w: 0, t: 0, l: 0 },
  );
  const hitRate = hist.length ? Math.round((stats.w / hist.length) * 100) : 0;

  /** 挂/卸一件道具：同类效果互斥（不在前端叠乘，服务端也只收一件） */
  function toggle(p: PropView) {
    setSel((cur) => {
      if (cur.includes(p.key)) return cur.filter((k) => k !== p.key);
      const same = props
        .filter((x) => x.effect === p.effect && cur.includes(x.key))
        .map((x) => x.key);
      return [...cur.filter((k) => !same.includes(k)), p.key];
    });
  }

  async function guess(g: "small" | "big") {
    if (busy) return;
    setBusy(true);
    setErr(null);
    setFlash(null);
    setRes(null);
    setNum(null);
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID
        ? crypto.randomUUID()
        : String(Date.now());
    try {
      const r = await api.post<GuessResult>("/api/v1/games/bigsmall", {
        bet,
        guess: g,
        props: sel,
        idempotency_key: idem.current,
      });
      setNum(r.number);
      const settle = () => {
        setRes(r);
        // 道具已消耗：清掉挂载态并刷新持有数（否则会显示一件已用完的道具）
        setSel([]);
        const hint =
          (r.props_applied?.length ?? 0) === 0
            ? ""
            : (r.shield_refund ?? 0) > 0
              ? ` · ${tg.propAppliedShield.replace(
                  "{n}",
                  String(r.shield_refund),
                )}`
              : r.effective_mult
                ? ` · ${tg.propAppliedMult.replace(
                    "{n}",
                    fmtMult(r.effective_mult),
                  )}`
                : "";
        const base = r.tie
          ? fmtCur(tg.tie, { n: r.number }, currency)
          : r.player_win
            ? fmtCur(tg.win, { n: r.number, net: r.net }, currency)
            : fmtCur(tg.lose, { n: r.number, net: -r.net }, currency);
        setFlash({
          kind: r.tie ? "tie" : r.player_win ? "win" : "lose",
          text: base + hint,
        });
        setStreak((s) => (r.player_win ? s + 1 : 0));
        setSessionPlays((n) => n + 1);
        setBusy(false);
        void loadMeta();
      };
      if (reduced) settle();
      else timer.current = window.setTimeout(settle, SPIN_MS);
    } catch (e) {
      // 1002 用后端具体原因（字典里的「参数校验失败」太笼统）
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

  return (
    <GameShell
      icon="🎯"
      title={tg.title}
      subtitle={`${tg.rule} · ${tg.winMult.replace("{n}", String(winMult))}`}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      notice={
        <PlayHint
          sessionPlays={sessionPlays}
          todayNet={ov?.me?.today_net ?? null}
        />
      }
      stage={
        <div className="flex w-full flex-col items-center gap-2">
          <GameStage
            art="/games/stage-bigsmall.jpg"
            className="h-[200px] w-full max-w-[460px]"
          >
            <RunwayOdometer number={num} spinning={busy} reduced={reduced} />
          </GameStage>
          {/* 双骰舞台（样图⑥）：两枚 72px 骰面，点数拆两枚显示 */}
          <div className="bs-dice" aria-hidden>
            {[
              Math.ceil((num ?? 2) / 2) || 1,
              Math.floor((num ?? 2) / 2) || 1,
            ].map((n, i) => (
                <DieFace key={i} n={n} />
              ),
            )}
          </div>
          <p className="bs-dice-cap">
            {num == null
              ? tg.bsDiceWait
              : `${tg.bsRecent.replace("{s}", "")} ${num}`}
          </p>
          <ResultFlash
            kind={flash?.kind ?? null}
            text={flash?.text ?? (busy ? tg.pending : null)}
          />
          {streak >= 3 && (
            <p className="text-xs font-bold text-[var(--warning)]">
              {tg.streak}
            </p>
          )}
          {hist.length > 0 && (
            <p className="bs-recent">
              {tg.bsRecent.replace(
                "{s}",
                hist
                  .slice(0, 5)
                  .map((h) =>
                    h.number == null
                      ? "·"
                      : h.number >= 11 && h.number <= 18
                        ? tg.bsPickBig
                        : tg.bsPickSmall,
                  )
                  .join(" "),
              )}
            </p>
          )}
          <SwStatRow
            items={[
              { lb: tg.bsStatPlays, vl: String(ov?.me?.today_plays ?? 0) },
              { lb: tg.bsStatMax, vl: `${fmtMult(winMult)}x`, tone: "gold" },
              {
                lb: tg.bsStatLeft,
                vl: ov?.me?.limit_left != null ? String(ov.me.limit_left) : "—",
                tone: "green",
              },
            ]}
          />
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className={PANEL_LG_COL}>
          <ChipSelect
            value={bet}
            onChange={setBet}
            maxBet={maxBet}
            disabled={busy}
          />
          {/* 三大选卡（样图⑥）：大 / 豹子 / 小。豹子只是展示位——
              玩法本体只有大/小两种下注，豹子按平局退款口径 */}
          <div className="bs-picks">
            <button
              type="button"
              onClick={() => guess("small")}
              disabled={busy}
              className="bs-pick"
            >
              <div className="bs-pick-t">{tg.bsPickSmall}</div>
              <div className="bs-pick-s">
                {tg.bsPickSmallSub.replace("{n}", fmtMult(winMult))}
              </div>
            </button>
            <button
              type="button"
              onClick={() => guess("big")}
              disabled={busy}
              className="bs-pick gold"
            >
              <div className="bs-pick-t">{tg.bsPickBig}</div>
              <div className="bs-pick-s">
                {tg.bsPickBigSub.replace("{n}", fmtMult(winMult))}
              </div>
            </button>
            <div className="bs-pick" aria-disabled="true">
              <div className="bs-pick-t">{tg.bsPickTriple}</div>
              <div className="bs-pick-s">{tg.bsPickTripleSub}</div>
            </div>
          </div>
          {/* 道具栏：只加权魔力的输赢，永不出物品 */}
          <PropBar
            props={props}
            sel={sel}
            busy={busy}
            onToggle={toggle}
          />
        </div>
      }
      side={
        <div className={PANEL_LG}>
          {/* 赔率表 */}
          <div
            className="mb-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3"
          >
            <h3 className="mb-2 text-xs font-bold text-sub">赔率表</h3>
            <div className="grid grid-cols-2 gap-1 text-[11px]">
              <div className="flex justify-between">
                <span className="text-sub">大 (11-18)</span>
                <b className="num text-[var(--sky-deep)]">
                  {fmtMult(winMult)}x
                </b>
              </div>
              <div className="flex justify-between">
                <span className="text-sub">小 (3-10)</span>
                <b className="num text-[var(--sky-deep)]">
                  {fmtMult(winMult)}x
                </b>
              </div>
              <div className="flex justify-between">
                <span className="text-sub">豹子 (三同)</span>
                <b className="num text-[var(--gold-deep)]">8x</b>
              </div>
              <div className="flex justify-between">
                <span className="text-sub">围骰 (全1/6)</span>
                <b className="num text-danger">15x</b>
              </div>
            </div>
          </div>
          <h2 className="mb-2 font-display text-base">{t.roadmap}</h2>
          <SwHist
            rows={hist.slice(0, 6).map((h) => ({
              t: (h.at ?? "").slice(11, 16) || "—",
              txt: h.net > 0 ? tg.bsHistWin : tg.bsHistLose,
              net: h.net,
            }))}
            empty={t.historyEmpty}
          />
          {hist.length > 0 && (
            <p className="num mt-2 text-[11px] text-sub">
              {tg.statLine
                .replace("{n}", String(hist.length))
                .replace("{w}", String(stats.w))
                .replace("{t}", String(stats.t))
                .replace("{l}", String(stats.l))
                .replace("{p}", String(hitRate))}
            </p>
          )}
          <p className="mt-2 text-[11px] text-sub">{tg.rule}</p>
        </div>
      }
    />
  );
}
