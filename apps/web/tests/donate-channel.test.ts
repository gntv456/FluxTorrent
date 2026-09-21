import { describe, expect, it } from "vitest";

/**
 * U4 §12.1：捐赠通道可用性判定逻辑单测。
 * 复刻 donate/_inner.tsx 的渲染条件（payment_enabled === false → 通道未开通横幅 + 提交禁用），
 * 后端 donate_state.payment_enabled = gateway.available() || FLUX_DEMO。
 */

function shouldShowChannelClosed(
  payment_enabled: boolean | undefined,
): boolean {
  // 与 _inner.tsx 一致：严格 false 才显示（undefined = 旧版 API 兼容，视为开通）
  return payment_enabled === false;
}

function submitDisabled(
  busy: boolean,
  amount: number,
  payment_enabled: boolean | undefined,
): boolean {
  // 与提交按钮 disabled 一致：busy/金额越界/通道关闭
  return busy || amount < 10 || amount > 66 || payment_enabled === false;
}

describe("donate payment channel gating (U4 §12.1)", () => {
  it("未配置网关（payment_enabled=false）显示「通道未开通」", () => {
    expect(shouldShowChannelClosed(false)).toBe(true);
  });

  it("已配置（true）或旧版缺省（undefined）不显示", () => {
    expect(shouldShowChannelClosed(true)).toBe(false);
    expect(shouldShowChannelClosed(undefined)).toBe(false);
  });

  it("通道关闭时提交按钮禁用（金额合法也不可提交）", () => {
    expect(submitDisabled(false, 20, false)).toBe(true);
  });

  it("通道开启且金额合法时可提交", () => {
    expect(submitDisabled(false, 20, true)).toBe(false);
    expect(submitDisabled(false, 20, undefined)).toBe(false);
  });

  it("金额越界仍然禁用", () => {
    expect(submitDisabled(false, 5, true)).toBe(true);
    expect(submitDisabled(false, 100, true)).toBe(true);
  });
});
