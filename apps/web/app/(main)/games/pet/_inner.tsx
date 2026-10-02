"use client";

import { useCallback, useEffect, useState } from "react";
import { PANEL_LG, PANEL_LG_COL } from "@/lib/ui-classes";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { GameShell } from "@/components/game/game-kit";
import { GameTabs } from "@/components/game/game-tabs";
import { PetLogPanel } from "@/components/game/pet-log";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { GameToast } from "@/components/game/game-kit-feedback";
import { PetCustom, PetPen, type PetStatus } from "@/components/game/pet-pen";
import { GameStage } from "@/components/game/game-stage";

export type { PetStatus };

/**
 * 宠物专注页：投喂 → 成长 → 领取产出。
 * 产出率恒 < 100%，宠物是把投喂的魔力「慢慢还一部分」的养成对象，不是套利工具。
 */
export default function PetFocusPage({
  initial,
}: {
  initial: PetStatus | null;
}) {
  const { dict, currency } = useI18n();
  const tp = dict.games.pet;
  const [st, setSt] = useState<PetStatus | null>(initial);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [tab, setTab] = useState("play");
  const [useCoupon, setUseCoupon] = useState(false);
  const [customOpen, setCustomOpen] = useState(false);
  // 余额条（反沉迷纪律：喂食扣钱前先看得见——与其它玩法页同口径）
  const [me, setMe] = useState<{
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  } | null>(null);
  // 投喂成功的「吃到东西」反馈：让宠物晃一下（纯前端，零账目）
  const [eating, setEating] = useState(false);
  // 「摸摸头」外部按钮 → PetPen 内部爱心动画的受控桥（替代 DOM 代理）
  const [patTick, setPatTick] = useState(0);
  const [toast, setToast] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);

  const newIdem = () =>
    typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : String(Date.now());

  // 余额条数据：喂食/领取后一并刷新（pending 入账会改变今日净收）
  const loadMe = useCallback(async () => {
    try {
      const r = await api.get<{ me?: typeof me }>("/api/v1/games");
      setMe(r.me ?? null);
    } catch {
      /* 取不到不影响玩法 */
    }
  }, []);

  useEffect(() => {
    void loadMe();
  }, [loadMe]);

  const fail = (e: unknown) =>
    setErr(
      e instanceof ApiError
        ? e.code === 1002
          ? e.message
          : (dict.errors[e.code] ?? e.message)
        : dict.common.networkError,
    );

  async function feed() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      const before = st;
      const r = await api.post<{
        spent: number;
        used_coupon: boolean;
        food_coupons: number;
        status: PetStatus;
      }>("/api/v1/games/pet/feed", {
        idempotency_key: newIdem(),
        use_coupon: useCoupon,
      });
      setSt({ ...r.status, food_coupons: r.food_coupons });
      void loadMe();
      setEating(true);
      window.setTimeout(() => setEating(false), 700);
      const leveled = before !== null && r.status.level > before.level;
      const gained = Math.max(0, r.status.exp - (before?.exp ?? 0));
      setToast({
        kind: leveled ? "jackpot" : "win",
        text: leveled
          ? tp.levelUp.replace("{n}", String(r.status.level))
          : r.used_coupon
            ? tp.fedCoupon.replace("{n}", String(gained))
            : tp.fed.replace("{n}", String(gained)),
      });
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  async function claim() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      const r = await api.post<{ earned: number; status: PetStatus }>(
        "/api/v1/games/pet/claim",
        { idempotency_key: newIdem() },
      );
      setSt(r.status);
      void loadMe();
      setToast({
        kind: r.earned > 0 ? "win" : "tie",
        text:
          r.earned > 0
            ? tp.claimed
                .replace("{n}", String(r.earned))
                .replace("{magic}", currency)
            : tp.claimNone,
      });
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  const feedCost = st?.feed_cost ?? 100;

  if (tab === "records") {
    return (
      <div className="flex flex-col gap-4">
        <GameTabs
          tabs={[
            { key: "play", label: dict.games.sub.tabPlay, icon: "🐾" },
            { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
          ]}
          active={tab}
          onChange={setTab}
        />
        <PetLogPanel />
      </div>
    );
  }
  return (
    <GameShell
      icon={<ArcadeGlyph k="pet" />}
      title={tp.title}
      subtitle={tp.sub}
      balance={me?.balance ?? null}
      todayNet={me?.today_net ?? null}
      limitLeft={me?.limit_left ?? null}
      stage={
        <div className="flex w-full flex-col items-center gap-3">
          <GameStage
            bg="radial-gradient(120% 130% at 50% 0%, #123a2e, #08170f 72%)"
            className="w-full max-w-[460px] py-6"
          >
            {st ? (
              <PetPen
                status={st}
                eating={eating}
                patTick={patTick}
                hungerLabel={tp.hunger}
                expLabel={tp.exp}
              />
            ) : (
              <p className="text-sm text-sub">{dict.common.networkError}</p>
            )}
          </GameStage>
          <GameToast message={toast} onDone={() => setToast(null)} />
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className={PANEL_LG_COL}>
          <GameTabs
            tabs={[
              { key: "play", label: dict.games.sub.tabPlay, icon: "🐾" },
              { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
            ]}
            active={tab}
            onChange={setTab}
          />
          {/* 当前宠物大卡（样图⑨）：名 / Lv / 亲密度口径（饱食度） */}
          <div className="pp-hero">
            <div className="pp-hero-face" aria-hidden>
              {st?.species === "cat"
                ? "🐱"
                : st?.species === "bunny"
                  ? "🐰"
                  : st?.species === "drake"
                    ? "🐲"
                    : "🥚"}
            </div>
            <div className="pp-hero-name">{st?.name ?? tp.title}</div>
            <div className="pp-hero-sub">
              Lv.{st?.level ?? 1} · {tp.hunger} {st?.hunger ?? 0}/100
            </div>
          </div>
          <div className="flex items-baseline justify-between gap-3">
            <span className="font-display text-lg">Lv.{st?.level ?? 1}</span>
            <span className="text-xs text-sub">
              {tp.yield.replace(
                "{n}",
                ((st?.yield_permille ?? 0) / 10).toFixed(0),
              )}
            </span>
          </div>
          <div className="pp-acts">
            <button
              type="button"
              onClick={feed}
              disabled={busy || !st || st.level >= (st?.max_level ?? 10)}
              className="pp-act sky"
            >
              {busy
                ? tp.feeding
                : useCoupon && (st?.food_coupons ?? 0) > 0
                  ? tp.feedCoupon
                  : tp.feed.replace("{n}", String(feedCost))}
            </button>
            <button
              type="button"
              onClick={() => setPatTick((n) => n + 1)}
              className="pp-act gold"
            >
              {tp.pat}
            </button>
            <button
              type="button"
              onClick={claim}
              disabled={busy || !st || st.pending <= 0}
              className="pp-act sky"
            >
              {busy ? tp.claiming : tp.claim}
            </button>
          </div>
          <label className="pp-coupon">
            <input
              type="checkbox"
              checked={useCoupon}
              onChange={(e) => setUseCoupon(e.target.checked)}
              disabled={!st || (st.food_coupons ?? 0) <= 0}
            />
            <span>
              {(st?.food_coupons ?? 0) > 0
                ? tp.coupon.replace("{n}", String(st?.food_coupons ?? 0))
                : tp.couponNone}
            </span>
          </label>
          <button
            type="button"
            className="pp-custom-btn"
            onClick={() => setCustomOpen((v) => !v)}
          >
            {tp.customBtn}
          </button>
          {customOpen && st && (
            <PetCustom
              status={st}
              onClose={() => setCustomOpen(false)}
              onSaved={(v) => {
                setSt({ ...st, name: v.name, species: v.species });
                setCustomOpen(false);
                setToast({ kind: "win", text: tp.saved });
              }}
            />
          )}
          {st && st.level >= st.max_level && (
            <p className="text-[11px] font-bold text-[var(--warning)]">
              {tp.full}
            </p>
          )}
        </div>
      }
      side={
        <div className={PANEL_LG}>
          {/* 我的宠舍（样图⑨）：当前物种占一格 + 虚线空位到 6 */}
          <div className="pp-pen-cap">{tp.penCap}</div>
          <div className="pp-pen-grid">
            <div className="pp-pen-cell on" aria-label={st?.name ?? ""}>
              {st?.species === "cat"
                ? "🐱"
                : st?.species === "bunny"
                  ? "🐰"
                  : st?.species === "drake"
                    ? "🐲"
                    : "🥚"}
            </div>
            {[2, 3, 4, 5, 6].map((i) => (
              <div key={i} className="pp-pen-cell empty" aria-hidden>
                +
              </div>
            ))}
          </div>
          <p className="mt-3 text-[11px] text-sub">
            {tp.rule.replace("{magic}", currency)}
          </p>
        </div>
      }
    />
  );
}
