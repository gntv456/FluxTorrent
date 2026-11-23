import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act } from "@testing-library/react";
import {
  START,
  stubApi,
  startOf,
  finishOf,
  mount,
  flushUntil,
} from "./mole-play-helpers";

/**
 * 打地鼠前端状态机回归闸门·结算域。
 * 开局/命中域在 mole-play.test.tsx；夹具在 mole-play-helpers.ts。
 *
 * 断言结算链：
 *   · 结算失败落 settled + err 可见且 auto 不续局
 *   · auto 连打：settled 后自动开局
 *   · stop 停 auto 并立即结算
 *   · 卸载兜底：局未结算时补一发 finish
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

describe("useMolePlay · settle", () => {
  it("结算失败：settled + err 可见，auto 不续局", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL) => {
        const path = String(input);
        if (path.endsWith("/mole/start")) {
          return {
            status: 200,
            headers: { get: () => "application/json" },
            json: async () => ({ code: 0, data: START, request_id: "t" }),
          } as unknown as Response;
        }
        return {
          status: 500,
          headers: { get: () => "application/json" },
          json: async () => ({
            code: 5000,
            message: "结算失败",
            request_id: "t",
          }),
        } as unknown as Response;
      }),
    );
    const { result } = mount();
    await act(async () => {
      await result.current.start();
    });
    act(() => {
      result.current.toggleAuto(); // 局中开 auto
    });
    await act(async () => {
      vi.advanceTimersByTime(START.round_ms + 200);
    });
    await flushUntil(() => result.current.phase === "settled");
    expect(result.current.err).toContain("结算失败");
    await act(async () => {
      vi.advanceTimersByTime(2000); // auto 续局窗口
    });
    await flushUntil(() => result.current.phase === "settled");
    expect(result.current.phase).toBe("settled"); // 失败不续局
  });

  it("auto 连打：settled 后自动开局", async () => {
    const second = { ...START, round_id: 2, spawns: [START.spawns[0]] };
    const calls = stubApi([START, second]);
    const { result } = mount();
    await act(async () => {
      void result.current.toggleAuto(); // idle 开 auto → 立即首局
    });
    await flushUntil(() => result.current.phase === "playing");
    await act(async () => {
      vi.advanceTimersByTime(START.round_ms + 200);
    });
    await flushUntil(() => result.current.phase === "settled");
    await act(async () => {
      vi.advanceTimersByTime(1400); // 1300ms 续局窗口
    });
    await flushUntil(() => startOf(calls).length === 2);
    await flushUntil(() => result.current.phase === "playing");
  });

  it("stop：停 auto 并立即结算进行中的局", async () => {
    const calls = stubApi([START]);
    const { result } = mount();
    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      void result.current.stop();
    });
    await flushUntil(() => result.current.phase === "settled");
    expect(result.current.auto).toBe(false);
    expect(finishOf(calls).length).toBeGreaterThanOrEqual(1);
  });

  it("卸载兜底：局未结算时补一发 finish", async () => {
    const calls = stubApi([START]);
    const { result, unmount } = mount();
    await act(async () => {
      await result.current.start();
    });
    unmount();
    await act(async () => {
      await Promise.resolve();
    });
    expect(finishOf(calls).length).toBeGreaterThanOrEqual(1);
  });
});
