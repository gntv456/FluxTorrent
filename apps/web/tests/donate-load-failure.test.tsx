import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import DonatePage from "@/app/(main)/donate/_inner";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 充值页取数失败不能演成「余额 0.00 USD + 空套餐」。
 *
 *  /donate/state 挂掉时旧实现 st=null，钱包按 `?? 0` 渲染成 0.00 USD，
 *  三区套餐全空——资金页上给出一个假余额，会员会真以为储值没了。
 */

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <DonatePage />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  document.cookie = "flux.session=test-token";
});
afterEach(() => {
  vi.unstubAllGlobals();
  document.cookie = "flux.session=; expires=Thu, 01 Jan 1970 00:00:00 GMT";
});

describe("充值页失败态", () => {
  it("状态取数失败：报加载失败，余额不再显示 0.00 USD", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("network down");
      }),
    );
    mount();
    expect((await screen.findAllByText(zhCN.common.loadFailed)).length)
      .toBeGreaterThan(0);
    // 余额格必须显示「—」：0.00 会被读成「储值没了」
    const label = await screen.findByText(zhCN.donate.balance);
    const box = label.parentElement?.textContent ?? "";
    expect(box).not.toContain("0.00");
    expect(box).toContain("—");
  });
});
