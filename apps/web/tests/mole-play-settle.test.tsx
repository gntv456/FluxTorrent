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
 *   · stop 只停 auto：局留给服务端按时结算（2026-11-24 契约：
 *     服务端拒绝未走完的 finish，切标签页不再白亏一局）
 *   · 卸载兜底：延迟到整局走完（+1s 余量）再补一发 finish
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

  it("stop：只停 auto，不提前终结进行中的局", async () => {
    const calls = stubApi([START]);
    const { result } = mount();
    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      void result.current.stop();
    });
    expect(result.current.auto).toBe(false);
    // 契约：服务端拒早结（schedule_done 闸），这一局还在走
    expect(result.current.phase).toBe("playing");
    expect(finishOf(calls).length).toBe(0);
  });

  it("卸载兜底：整局走完后才补 finish，半途不发", async () => {
    const calls = stubApi([START]);
    const { result, unmount } = mount();
    await act(async () => {
      await result.current.start();
    });
    unmount();
    await act(async () => {
      await Promise.resolve();
    });
    // 卸载当下：半途不发 finish（服务端会拒）
    expect(finishOf(calls).length).toBe(0);
    // 推进到整局走完 + 1s 余量后：补一发
    await act(async () => {
      vi.advanceTimersByTime(31_000 + 1_000 + 100);
      await Promise.resolve();
    });
    expect(finishOf(calls).length).toBe(1);
    void result;
  });
});
