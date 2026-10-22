import { describe, it, expect } from "vitest";

import { pushTestMessage } from "@/lib/push-test-result";

/**
 * P-570：/push/test 的读数必须诚实。
 *
 * 旧实现把 ServiceError 并进 delivered、界面又不读响应体，于是一次
 * 根本没送达的测试也显示「测试消息已发送」。服务端现在分三档计数，
 * 这里钉住「什么读数该说什么话」。
 */

const t = {
  testSent: "sent",
  testPartial: "partial",
  testFailed: "failed",
  testExpired: "expired",
};

describe("推送测试读数回话", () => {
  it("全部送达才说「已发送」", () => {
    expect(
      pushTestMessage({ delivered: 2, failed: 0, expired_cleaned: 0 }, t),
    ).toBe("sent");
  });

  it("P-570 正控：只有失败时不许说已发送", () => {
    expect(
      pushTestMessage({ delivered: 0, failed: 1, expired_cleaned: 0 }, t),
    ).toBe("failed");
  });

  it("订阅已失效（Gone）要让人重新开启推送", () => {
    expect(
      pushTestMessage({ delivered: 0, failed: 0, expired_cleaned: 1 }, t),
    ).toBe("expired");
  });

  it("部分失败或失效不冒充全成功", () => {
    expect(
      pushTestMessage({ delivered: 1, failed: 1, expired_cleaned: 0 }, t),
    ).toBe("partial");
    expect(
      pushTestMessage({ delivered: 1, failed: 0, expired_cleaned: 1 }, t),
    ).toBe("partial");
  });
});
