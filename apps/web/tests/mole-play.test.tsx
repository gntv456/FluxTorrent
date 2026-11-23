import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act } from "@testing-library/react";
import {
  START,
  SETTLED,
  stubApi,
  startOf,
  finishOf,
  mount,
  flushUntil,
} from "./mole-play-helpers";

/**
 * 打地鼠前端状态机（useMolePlay）回归闸门·开局与命中域。
 * 结算域在 mole-play-settle.test.tsx；夹具与坑位注记在 mole-play-helpers.ts。
 *
 * 断言局内契约：
 *   · 开局带幂等键；busy 闸防双击；下一局换新键
 *   · 排程按 at 冒头 / 按 at+up 缩头；空洞计 miss 不发请求
 *   · 炸弹扣分 + combo 清零 + bombs 单列
 */

beforeEach(() => {
  vi.useFakeTimers();
  document.cookie = "flux.session=1; path=/";
  document.cookie = "flux_csrf=tok-for-test; path=/";
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("useMolePlay · start & whack", () => {
  it("开局带幂等键，busy 闸防双击", async () => {
    const calls = stubApi([START]);
    const { result } = mount();
    const ps: Promise<void>[] = [];
    await act(async () => {
      ps.push(result.current.start());
      ps.push(result.current.start()); // busy 中的第二次点击：闸在 ref 上，同步返回
    });
    await act(async () => {
      await Promise.all(ps);
    });
    // busy 闸的现场：两次调用只有一次 /mole/start 出网
    const starts = startOf(calls);
    expect(starts).toHaveLength(1);
    expect(String(starts[0].body.idempotency_key)).toBeTruthy();
    expect(result.current.phase).toBe("playing");
  });

  it("下一局换新幂等键", async () => {
    const second = { ...START, round_id: 2, spawns: [START.spawns[0]] };
    const calls = stubApi([START, second]);
    const { result } = mount();
    await act(async () => {
      await result.current.start();
    });
    const firstKey = String(startOf(calls)[0].body.idempotency_key);
    await act(async () => {
      vi.advanceTimersByTime(START.round_ms + 200);
    });
    await flushUntil(() => result.current.phase === "settled");
    await act(async () => {
      await result.current.start();
    });
    const starts = startOf(calls);
    expect(starts).toHaveLength(2);
    expect(String(starts[1].body.idempotency_key)).not.toBe(firstKey);
  });

  it("排程按 at 冒头、at+up 缩头；hit 当场落账", async () => {
    const calls = stubApi([START]);
    const h = mount();
    await act(async () => {
      await h.result.current.start();
    });
    expect(h.result.current.phase).toBe("playing");
    await act(async () => {
      vi.advanceTimersByTime(1); // seq0 at=0：冒头
    });
    expect(h.result.current.active[0]).toBeTruthy();
    act(() => {
      h.result.current.whack(0);
    });
    expect(h.result.current.score).toBe(50);
    expect(h.result.current.hits).toBe(1);
    expect(h.result.current.active[0]).toBeUndefined();
    await flushUntil(
      () => calls.filter((c) => c.path.endsWith("/mole/hit")).length === 1,
    );
    const hit = calls.filter((c) => c.path.endsWith("/mole/hit"))[0];
    expect(hit.body).toEqual({ round_id: 1, seq: 0 });
    await act(async () => {
      vi.advanceTimersByTime(1501); // up 后缩头
    });
    expect(h.result.current.active[0]).toBeUndefined();
  });

  it("缩头后洞空：再来一锤计 miss", async () => {
    stubApi([START]);
    const h = mount();
    await act(async () => {
      await h.result.current.start();
    });
    await act(async () => {
      vi.advanceTimersByTime(1501); // 第一只已缩头
    });
    act(() => {
      h.result.current.whack(0);
    });
    expect(h.result.current.miss).toBe(1);
    expect(h.result.current.score).toBe(0);
  });

  it("空洞挥锤计 miss 且不发请求", async () => {
    const calls = stubApi([START]);
    const h = mount();
    await act(async () => {
      await h.result.current.start();
    });
    act(() => {
      h.result.current.whack(2); // 第一只在洞 0
    });
    expect(h.result.current.miss).toBe(1);
    expect(h.result.current.score).toBe(0);
    expect(calls.filter((c) => c.path.endsWith("/mole/hit"))).toHaveLength(0);
  });

  it("炸弹：扣分 + combo 清零 + bombs 单列", async () => {
    stubApi([START]);
    const h = mount();
    await act(async () => {
      await h.result.current.start();
    });
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    act(() => {
      h.result.current.whack(0); // 灰鼠 +50
    });
    // 夹具：炸弹 = seq2（at=3800、洞 2）。逐段推进防踩空
    await act(async () => {
      vi.advanceTimersByTime(1900); // seq1 冒头（洞 1，普通鼠，不敲）
    });
    await act(async () => {
      vi.advanceTimersByTime(1900); // seq2（炸弹）冒头
    });
    expect(h.result.current.active[2]).toBeTruthy();
    act(() => {
      h.result.current.whack(2); // 炸弹 -25
    });
    expect(h.result.current.score).toBe(25);
    expect(h.result.current.combo).toBe(0);
    expect(h.result.current.bombs).toBe(1);
    expect(h.result.current.hits).toBe(1); // 炸弹不进 hits
  });

  it("结算以服务端 score 为准：乐观分被纠正", async () => {
    const calls = stubApi([START], { ...SETTLED, score: 0, payout: 0 });
    const { result } = mount();
    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    act(() => {
      result.current.whack(0); // 本地 +50
    });
    await act(async () => {
      vi.advanceTimersByTime(1900);
    });
    act(() => {
      result.current.whack(1); // 本地 -25 → 25
    });
    await act(async () => {
      vi.advanceTimersByTime(START.round_ms + 200);
    });
    await flushUntil(() => result.current.phase === "settled");
    expect(result.current.score).toBe(0); // 服务端说 0：本地分被纠正
    expect(result.current.res?.payout).toBe(0);
    expect(finishOf(calls)).toHaveLength(1);
    expect(finishOf(calls)[0].body).toEqual({ round_id: 1 });
  });

  it("开局失败：停在 idle、err 可见、可重试", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        ({
          status: 400,
          headers: { get: () => "application/json" },
          json: async () => ({
            code: 4001,
            message: "票价超过上限",
            request_id: "t",
          }),
        }) as unknown as Response,
      ),
    );
    const { result } = mount();
    await act(async () => {
      void result.current.start();
    });
    await flushUntil(() => result.current.err !== null);
    expect(result.current.phase).toBe("idle");
    expect(result.current.err).toContain("票价超过上限");
    const calls = stubApi([START]);
    await act(async () => {
      await result.current.start();
    });
    expect(result.current.phase).toBe("playing");
    expect(startOf(calls)).toHaveLength(1);
  });
});
