"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "@/lib/api-client";
import { clientUuid } from "@/lib/client-uuid";

/** 一只冒头的地鼠：`idx` 是奖池下标，-1 是炸弹。 */
export interface MoleSpawn {
  seq: number;
  at: number;
  up: number;
  hole: number;
  idx: number;
  rarity: number;
  label: string;
  value: number;
}

export interface MoleStart {
  round_id: number;
  ticket: number;
  round_ms: number;
  spawns: MoleSpawn[];
  hit_early_ms: number;
  hit_late_ms: number;
}

export interface MoleResult {
  round_id: number;
  score: number;
  payout: number;
  net: number;
  ticket: number;
  hits: number;
  bombs: number;
  best: number;
}

type Phase = "idle" | "playing" | "settling" | "settled";

/** 一局「走完」的客户端估算：整局 30s + 1s 余量（与后端时长闸同向，
 *  宁可晚一秒发 finish，不抢在闸前被拒）。服务端还有补结算兜底，
 *  这里只是让玩家离开页面时也能尽快拿到结算。 */
const ROUND_DONE_MS = 31_000;

/**
 * 打地鼠一局的状态机：开局拿排程 → 按 at 让地鼠冒头 → 敲击当场回传 →
 * 倒计时结束结算。
 *
 * 分数先乐观累加（手感），到 finish 以**服务端**的 score 为准：
 * 晚到窗口的锤子服务端不认，本地那几分会被结算纠正。
 */
export function useMolePlay(opts: {
  describeError: (e: unknown) => string;
  onSettled: () => void;
}) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [left, setLeft] = useState(0);
  const [score, setScore] = useState(0);
  const [hits, setHits] = useState(0);
  const [bombs, setBombs] = useState(0);
  const [combo, setCombo] = useState(0);
  const [active, setActive] = useState<Record<number, MoleSpawn>>({});
  const [res, setRes] = useState<MoleResult | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [auto, setAuto] = useState(false);
  const [sessionPlays, setSessionPlays] = useState(0);
  const [miss, setMiss] = useState(0);

  const descRef = useRef(opts.describeError);
  const settledRef = useRef(opts.onSettled);
  const roundRef = useRef<{
    id: number;
    t0: number;
    timers: number[];
    ticks: number[];
  } | null>(null);
  const activeRef = useRef<Record<number, MoleSpawn>>({});
  const phaseRef = useRef<Phase>("idle");
  const autoRef = useRef(false);
  const busyRef = useRef(false);
  const idemRef = useRef("");
  const autoTimer = useRef<number | null>(null);
  const startRef = useRef<(() => Promise<void>) | null>(null);

  useEffect(() => {
    descRef.current = opts.describeError;
    settledRef.current = opts.onSettled;
  }, [opts.describeError, opts.onSettled]);

  const toPhase = (p: Phase) => {
    phaseRef.current = p;
    setPhase(p);
  };
  const setActiveMap = (next: Record<number, MoleSpawn>) => {
    activeRef.current = next;
    setActive(next);
  };
  const clearTimers = useCallback(() => {
    const r = roundRef.current;
    if (!r) return;
    for (const t of r.timers) window.clearTimeout(t);
    for (const t of r.ticks) window.clearInterval(t);
    r.timers = [];
    r.ticks = [];
  }, []);

  const finish = useCallback(
    async (retry = false) => {
      const r = roundRef.current;
      if (!r || phaseRef.current !== "playing") return;
      clearTimers();
      toPhase("settling");
      setActiveMap({});
      try {
        const out = await api.post<MoleResult>(
          "/api/v1/games/mole/finish",
          { round_id: r.id },
        );
        setScore(out.score);
        setRes(out);
        setSessionPlays((n) => n + 1);
        toPhase("settled");
        settledRef.current();
        if (autoRef.current) {
          autoTimer.current = window.setTimeout(() => {
            void startRef.current?.();
          }, 1300);
        }
      } catch (e) {
        // 「还没结束」是时钟边界（客户端 t0 晚于服务端 created_at 时，
        // finish 可能抢在时长闸前 1~2 秒）：等过闸再试一次，别把
        // 一局有分数的玩家晾在没有结算结果的空屏上。
        if (!retry) {
          window.setTimeout(() => {
            toPhase("playing");
            void finishRef.current?.(true);
          }, 2_500);
          return;
        }
        setErr(descRef.current(e));
        setRes(null);
        toPhase("settled");
      }
    },
    [clearTimers],
  );

  const start = useCallback(async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setErr(null);
    setRes(null);
    setScore(0);
    setHits(0);
    setBombs(0);
    setCombo(0);
    setMiss(0);
    setActiveMap({});
    if (!idemRef.current) idemRef.current = clientUuid();
    try {
      const s = await api.post<MoleStart>(
        "/api/v1/games/mole/start",
        { idempotency_key: idemRef.current },
      );
      if (s.spawns.length === 0) {
        setErr(descRef.current(new Error("empty")));
        return;
      }
      idemRef.current = "";
      clearTimers();
      const r: {
        id: number;
        t0: number;
        timers: number[];
        ticks: number[];
      } = { id: s.round_id, t0: Date.now(), timers: [], ticks: [] };
      roundRef.current = r;
      setLeft(s.round_ms);
      toPhase("playing");
      for (const sp of s.spawns) {
        r.timers.push(
          window.setTimeout(() => {
            setActiveMap({ ...activeRef.current, [sp.hole]: sp });
          }, sp.at),
        );
        r.timers.push(
          window.setTimeout(() => {
            const cur = { ...activeRef.current };
            if (cur[sp.hole]?.seq === sp.seq) delete cur[sp.hole];
            setActiveMap(cur);
          }, sp.at + sp.up),
        );
      }
      r.timers.push(
        window.setTimeout(() => void finishRef.current?.(), s.round_ms + 150),
      );
      r.ticks.push(
        window.setInterval(() => {
          const cur = roundRef.current;
          if (!cur) return;
          setLeft(Math.max(0, s.round_ms - (Date.now() - cur.t0)));
        }, 100),
      );
    } catch (e) {
      /* 开局失败不自动重试：余额/额度/网络都要人先看一眼 */
      setErr(descRef.current(e));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }, [clearTimers]);

  const finishRef = useRef<((retry?: boolean) => Promise<void>) | null>(null);
  finishRef.current = finish;
  startRef.current = start;

  const whack = useCallback((hole: number) => {
    if (phaseRef.current !== "playing") return;
    const sp = activeRef.current[hole];
    if (!sp) {
      setMiss((n) => n + 1);
      return;
    }
    const cur = { ...activeRef.current };
    delete cur[hole];
    setActiveMap(cur);
    setHits((n) => (sp.value < 0 ? n : n + 1));
    setCombo((c) => (sp.value < 0 ? 0 : c + 1));
    // 乐观分数与结算同向钳 0：服务端 payout = max(score, 0)，此前炸弹
    // 能把本地分数打成负数、结算又弹回 0，HUD 上像「分数凭空消失」。
    setScore((v) => Math.max(0, v + sp.value));
    if (sp.value < 0) setBombs((n) => n + 1);
    const rid = roundRef.current?.id;
    if (!rid) return;
    void api
      .post("/api/v1/games/mole/hit", { round_id: rid, seq: sp.seq })
      .catch(() => {
        /* 单锤失败不打断整局：窗口外的锤子服务端本来就不认 */
      });
  }, []);

  const toggleAuto = useCallback(() => {
    const next = !autoRef.current;
    autoRef.current = next;
    setAuto(next);
    if (next && phaseRef.current === "idle") void start();
  }, [start]);

  const stop = useCallback(() => {
    // 只停自动模式，**不**再提前 finish：切标签页时这一局还在走，
    // 服务端拒绝未走完的结算（schedule_done 闸），回来还能接着打；
    // 不回来的话，下次开局前服务端会补结算（mole_start 的补结算口）。
    autoRef.current = false;
    setAuto(false);
    if (autoTimer.current !== null) {
      window.clearTimeout(autoTimer.current);
      autoTimer.current = null;
    }
  }, []);

  useEffect(() => {
    return () => {
      clearTimers();
      if (autoTimer.current !== null) {
        window.clearTimeout(autoTimer.current);
      }
      const r = roundRef.current;
      if (r && phaseRef.current !== "idle" && phaseRef.current !== "settled") {
        // 卸载（离开页面）时把「距离这一局走完还差多久」交给一个定时器，
        // 到点再调 finish；页面直接关掉的话定时器随页死，交给下次开局的
        // 补结算。此前这里立刻 finish，半局被当场结掉（后端现在会拒）。
        const leftMs = Math.max(
          0,
          r.t0 + ROUND_DONE_MS - Date.now(),
        );
        window.setTimeout(() => {
          void api
            .post("/api/v1/games/mole/finish", { round_id: r.id })
            .catch(() => {});
        }, leftMs + 1_000);
      }
    };
    // ROUND_DONE_MS：最后一只收尾 ≈ 27.7s，取整局 30s + 网络余量 1s
    // （与后端 settled_or_over 的时长闸同向：宁可晚一秒，不抢在闸前）。
  }, [clearTimers]);

  return {
    phase, left, score, hits, bombs, combo, active, res, err, busy, auto,
    sessionPlays, miss, start, whack, finish, toggleAuto, stop,
  };
}
