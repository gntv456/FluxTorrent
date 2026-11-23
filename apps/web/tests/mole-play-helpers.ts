import { vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  useMolePlay,
  type MoleSpawn,
  type MoleStart,
  type MoleResult,
} from "@/components/game/use-mole-play";

/**
 * mole-play 系列测试的共享夹具（只被 tests/mole-play*.test.tsx 引用）。
 *
 * 排程/结算形状与后端 build_schedule / finish_view 同形；fetch stub
 * 按路径回放。坑（实测记录在 mole-play.test.tsx 头注）也在这里统一兜。
 */

export function spawn(seq: number, over: Partial<MoleSpawn> = {}): MoleSpawn {
  return {
    seq,
    at: seq * 1900,
    up: 1500,
    hole: seq % 3,
    idx: 1,
    rarity: 1,
    label: "灰鼠",
    value: 50,
    ...over,
  };
}

export const START: MoleStart = {
  round_id: 1,
  ticket: 100,
  round_ms: 30_000,
  spawns: [
    spawn(0),
    spawn(1),
    spawn(2, { value: -25, label: "炸弹", idx: -1 }),
  ],
  hit_early_ms: 300,
  hit_late_ms: 500,
};

export const SETTLED: MoleResult = {
  round_id: 1,
  score: 50,
  payout: 50,
  net: -50,
  ticket: 100,
  hits: 2,
  bombs: 1,
  best: 50,
};

export type Call = { path: string; body: Record<string, unknown> };

/** 极简 fetch stub：按路径回放 JSON；记录全部调用顺序。 */
export function stubApi(
  starts: MoleStart[],
  settled: MoleResult | null = SETTLED,
) {
  const calls: Call[] = [];
  let startIdx = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input).replace(/^https?:\/\/[^/]+/, "");
      const body = init?.body
        ? (JSON.parse(String(init.body)) as Record<string, unknown>)
        : {};
      calls.push({ path, body });
      let payload: unknown;
      if (path.endsWith("/mole/start")) {
        payload = starts[Math.min(startIdx, starts.length - 1)];
        startIdx += 1;
      } else if (path.endsWith("/mole/finish")) {
        payload = settled;
      } else {
        payload = { hit: true, value: 0 };
      }
      return {
        status: 200,
        headers: { get: () => "application/json" },
        json: async () => ({ code: 0, data: payload, request_id: "t" }),
      } as unknown as Response;
    }),
  );
  return calls;
}

export const startOf = (calls: Call[]) =>
  calls.filter((c) => c.path.endsWith("/mole/start"));
export const finishOf = (calls: Call[]) =>
  calls.filter((c) => c.path.endsWith("/mole/finish"));

export function mount() {
  return renderHook(() =>
    useMolePlay({
      describeError: (e) => String(e),
      onSettled: () => {},
    }),
  );
}

/** 微任务 flush + 定点推进交替，直到条件成立（等待异步收尾用）。 */
export async function flushUntil(cond: () => boolean, stepMs = 25) {
  for (let i = 0; i < 200; i += 1) {
    await actFlush();
    if (cond()) return;
    await actStep(stepMs);
  }
  throw new Error("flushUntil 自旋耗尽（条件未成立）");
}

export async function actFlush() {
  await act(async () => {
    await Promise.resolve();
  });
}

export async function actStep(ms: number) {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
}
