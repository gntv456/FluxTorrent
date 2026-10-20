import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, renderHook, cleanup, act } from "@testing-library/react";
import { readFileSync } from "node:fs";
import path from "node:path";
import {
  SPARK_EVENT,
  SPARK_WRITE_PATHS,
  notifySparkChanged,
  useSparkSync,
} from "@/lib/spark-event";

/**
 * 「写魔力之后，顶栏要跟着变」的回归闸门。
 *
 * 背景（2026-10-21 以普通用户身份实测）：首页「立即签到」点下去后端真的入账
 * （`/attendance/checkin` 回 code:0 reward:8，库里 spark_balance 8339→8347、
 * attendance 落了今天），但页面不刷新时界面上的魔力读数仍停在 8,339。
 * 根因不是签到一处：顶栏 chip 的余额是 `user-menu` 进页面时
 * `useEffect(…, [])` 读 `/me`+`/me/spark` 存的本地 state，
 * **`router.refresh()` 不会重跑它** ⇒ 那些靠 refresh 的口照样刷新不了数字；
 * 而后端会动余额的会员侧写口实测有 68 个（商城/勋章/邀请/银行/农场/游戏/
 * 付费种下载链…），逐个手挂必漏。
 *
 * 修法落在唯一的必经口 `lib/api-client.ts` 的 `request()`：写方法 + 命中
 * 登记表 ⇒ 广播 `SPARK_EVENT`；顶栏 `useSparkSync(refreshSpark)` 回读。
 * 登记表与后端余额原语的对账由 `scripts/pt_audit_spark_sync.py` 常驻守住。
 *
 * 这里全是**接线级**断言：真发一次请求看有没有广播，不比对源码文本。
 */

const seen: number[] = [];
function spy() {
  seen.length = 0;
  const h = () => seen.push(1);
  window.addEventListener(SPARK_EVENT, h);
  return () => window.removeEventListener(SPARK_EVENT, h);
}

function mockFetch(handler: (url: string, init?: RequestInit) => unknown) {
  const fn = vi.fn(async (input: unknown, init?: RequestInit) => {
    const url = String(input);
    const body = handler(url, init);
    return {
      status: 200,
      headers: new Map([["content-type", "application/json"]]) as unknown as
        Headers,
      json: async () => body,
      text: async () => JSON.stringify(body),
    } as unknown as Response;
  });
  // 让 headers.get 可用
  (fn as unknown as { _m: typeof fn })._m = fn;
  vi.stubGlobal("fetch", fn);
  return fn;
}

beforeEach(() => {
  cleanup();
  document.cookie = "flux.session=1; path=/";
  document.cookie = "flux_csrf=tok-for-test; path=/";
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("顶栏余额广播", () => {
  it("P1 登记表覆盖到的写口，成功后广播一次", async () => {
    const off = spy();
    mockFetch(() => ({ code: 0, message: "ok", data: { ok: true } }));
    const { api } = await import("@/lib/api-client");
    await api.post("/api/v1/shop/buy", { item: 1 });
    expect(seen.length).toBe(1);
    off();
  });

  it("P2 GET 不广播（读口不该触发回读）", async () => {
    const off = spy();
    mockFetch(() => ({ code: 0, message: "ok", data: { balance: 1 } }));
    const { api } = await import("@/lib/api-client");
    await api.get("/api/v1/me/spark");
    expect(seen.length).toBe(0);
    off();
  });

  it("P3 未登记的写口不广播（表是白名单而非全量刷新）", async () => {
    const off = spy();
    mockFetch(() => ({ code: 0, message: "ok", data: {} }));
    const { api } = await import("@/lib/api-client");
    await api.post("/api/v1/me/title", { title: "x" });
    expect(seen.length).toBe(0);
    off();
  });

  it("P4 登记表里的 `{id}` 段按通配命中真实路径", async () => {
    const off = spy();
    mockFetch(() => ({ code: 0, message: "ok", data: {} }));
    const { api } = await import("@/lib/api-client");
    await api.post("/api/v1/torrents/50122/download-link", {});
    expect(seen.length).toBe(1);
    off();
  });

  it("P5 查询串不参与匹配，仍算命中", async () => {
    const off = spy();
    mockFetch(() => ({ code: 0, message: "ok", data: {} }));
    const { api } = await import("@/lib/api-client");
    await api.post("/api/v1/games/bigsmall?x=1", {});
    expect(seen.length).toBe(1);
    off();
  });

  it("P6 业务失败（code≠0）不广播——钱没扣成，不该假装刷新", async () => {
    const off = spy();
    mockFetch(() => ({ code: 5002, message: "已经签到过啦", data: null }));
    const { api } = await import("@/lib/api-client");
    await expect(api.post("/api/v1/attendance/checkin", {}))
      .rejects.toThrow();
    expect(seen.length).toBe(0);
    off();
  });

  it("P7 登记表自身形状可靠：无空串、无重复、无 /api/v1 前缀混入", () => {
    expect(SPARK_WRITE_PATHS.length).toBeGreaterThan(40);
    expect(new Set(SPARK_WRITE_PATHS).size).toBe(SPARK_WRITE_PATHS.length);
    for (const p of SPARK_WRITE_PATHS) {
      expect(p.startsWith("/")).toBe(true);
      expect(p.startsWith("/api/v1")).toBe(false);
    }
  });

  it("P8 订阅方（顶栏那种）收到广播会重读余额，卸载后不再重读", () => {
    const on = vi.fn();
    const { unmount } = renderHook(() => useSparkSync(on));
    act(() => notifySparkChanged());
    expect(on).toHaveBeenCalledTimes(1);
    unmount();
    act(() => notifySparkChanged());
    expect(on).toHaveBeenCalledTimes(1);
  });

  it("P9 原生 dispatch 也算一次广播（不只认封装函数）", () => {
    const on = vi.fn();
    renderHook(() => useSparkSync(on));
    act(() => {
      window.dispatchEvent(new Event(SPARK_EVENT));
    });
    expect(on).toHaveBeenCalledTimes(1);
  });

  it("P10 回调换引用不重复绑定监听（防一次广播发两趟请求）", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHook(
      ({ cb }: { cb: () => void }) => useSparkSync(cb),
      { initialProps: { cb: first } }
    );
    rerender({ cb: second });
    act(() => notifySparkChanged());
    expect(second).toHaveBeenCalledTimes(1);
    expect(first).not.toHaveBeenCalled();
  });

  it("P11 SparkBridge 收到广播会 router.refresh()（SSR 直出的余额靠这条）", async () => {
    const refresh = vi.fn();
    vi.doMock("next/navigation", () => ({ useRouter: () => ({ refresh }) }));
    const { SparkBridge } = await import("@/components/spark-bridge");
    render(<SparkBridge />);
    act(() => notifySparkChanged());
    expect(refresh).toHaveBeenCalledTimes(1);
    vi.doUnmock("next/navigation");
  });

  it("P12 根 layout 真的挂了 SparkBridge（否则 P11 的组件没人渲染）", () => {
    const src = readFileSync(
      path.resolve(process.cwd(), "app", "layout.tsx"), "utf-8")
      .split("\n")
      .filter((l) => !/^\s*(\/\/|\*|\/\*)/.test(l))
      .join("\n");
    expect(src).toContain("<SparkBridge");
    expect(src).toContain('from "@/components/spark-bridge"');
  });
});
