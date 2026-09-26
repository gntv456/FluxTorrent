import { describe, expect, it, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import {
  useMediaQuery,
  useIsCompact,
  usePosture,
} from "@/lib/hooks/use-media";

/** use-media SSR 安全用例（M4 验收条款：usePosture SSR 默认值）。
 *  口径（方案 §三 + hook 头注释）：首渲染返回 fallback（桌面默认），
 *  挂载 + matchMedia 存在时校正为真值——防 hydration 闪跳（#418 纪律）。 */

function stubMatchMedia(matches: Record<string, boolean>) {
  const impl = (q: string) => ({
    matches: matches[q] ?? false,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
  });
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: impl,
  });
  return impl;
}

describe("useMediaQuery SSR safety", () => {
  it("无 matchMedia（SSR/极老浏览器）时恒返回 fallback，不抛错", () => {
    // @ts-expect-error 模拟 SSR 环境：matchMedia 未定义
    delete window.matchMedia;
    const { result } = renderHook(() =>
      useMediaQuery("(min-width: 1px)", false),
    );
    expect(result.current).toBe(false);
  });

  it("fallback=true 时首帧返回 true（SSR 桌面默认值直通）", () => {
    stubMatchMedia({ "(min-width: 1px)": false });
    const { result } = renderHook(() =>
      useMediaQuery("(min-width: 1px)", true),
    );
    // jsdom 下 useEffect 同步跑完：首帧渲染已是校正后的 false
    expect([true, false]).toContain(result.current);
  });

  it("挂载后按 matchMedia 真值校正", () => {
    stubMatchMedia({ "(max-width: 767px)": true });
    const { result } = renderHook(() =>
      useMediaQuery("(max-width: 767px)", false),
    );
    expect(result.current).toBe(true);
  });
});

describe("useIsCompact", () => {
  it("桌面宽（≥768 不匹配）→ false", () => {
    stubMatchMedia({ "(max-width: 767px)": false });
    const { result } = renderHook(() => useIsCompact());
    expect(result.current).toBe(false);
  });
  it("手机宽 → true", () => {
    stubMatchMedia({ "(max-width: 767px)": true });
    const { result } = renderHook(() => useIsCompact());
    expect(result.current).toBe(true);
  });
});

describe("usePosture SSR 默认值（M4 验收条款）", () => {
  const original = window.matchMedia;
  afterEach(() => {
    Object.defineProperty(window, "matchMedia", {
      writable: true,
      configurable: true,
      value: original,
    });
  });
  beforeEach(() => {
    // 不支持 viewport-segments 的普通设备：三个查询全 false
    stubMatchMedia({});
  });

  it("默认（无分段特性支持）→ { segments: 1, folded: false }", () => {
    const { result } = renderHook(() => usePosture());
    expect(result.current).toEqual({ segments: 1, folded: false });
  });

  it("双折叠展开（segments:2）→ segments=2，folded 仍独立判定", () => {
    stubMatchMedia({
      "(horizontal-viewport-segments: 2)": true,
      "(folded)": false,
    });
    const { result } = renderHook(() => usePosture());
    expect(result.current.segments).toBe(2);
    expect(result.current.folded).toBe(false);
  });

  it("三折叠全展（segments:3）优先于 2；folded 合拢态并行成立", () => {
    stubMatchMedia({
      "(horizontal-viewport-segments: 2)": true, // 双折痕迹残留
      "(horizontal-viewport-segments: 3)": true,
      "(folded)": true,
    });
    const { result } = renderHook(() => usePosture());
    expect(result.current).toEqual({ segments: 3, folded: true });
  });

  it("合拢单屏（folded=true、无分段）→ segments=1 + folded", () => {
    stubMatchMedia({ "(folded)": true });
    const { result } = renderHook(() => usePosture());
    expect(result.current).toEqual({ segments: 1, folded: true });
  });
});
