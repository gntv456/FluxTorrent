import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

import { PromoBuyButton } from "@/components/promo-buy-button";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-628：详情页「购买置顶免费」两处界面缺陷。
 * ① 入口不看审批状态：待审/被拒/暂缓种（approval_status≠1）照样显示，
 *    POST /promo/buy 在扣款前回 404「资源不存在」⇒ 点进去是死路
 *    （现网 /torrent/51609 实测：弹窗选档后回「资源不存在」）。
 * ② 档位价格排版把价目塞进了货币名模板 `common.spark="{magic}"`，
 *    现网读数「24h · 5000 5000」——价格重复且没有货币名。
 * 负控：翻回 HEAD 版本 ⇒ 第一条红在入口仍在、第二条红在读数重复。
 */

const { get, post } = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: vi.fn(), push: vi.fn() }),
}));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post } };
});

const PLANS = {
  enabled: true,
  plans: {
    sticky1: [{ hours: 24, price: 5000 }],
    free: [{ hours: 24, price: 8000 }],
  },
  kinds: [
    {
      kind: "sticky1",
      label_zh: "一级置顶",
      label_en: "Sticky 1",
      effect: "sticky1",
      i18n_key: null,
    },
    {
      kind: "free",
      label_zh: "限时免费",
      label_en: "Free",
      effect: "free",
      i18n_key: null,
    },
  ],
};

function mount(approved: boolean) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN" currency="魔力">
      <PromoBuyButton torrentId={42} approved={approved} />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
  get.mockResolvedValue(PLANS);
  post.mockReset();
});

describe("P-628 促销购买入口", () => {
  it("待审/被拒种不渲染入口，也不白请求价目表", () => {
    mount(false);
    expect(
      screen.queryByRole("button", { name: /购买置顶免费/ }),
    ).toBe(null);
    expect(get).not.toHaveBeenCalled();
  });

  it("已过审种：档位价带货币名且不重复", async () => {
    mount(true);
    const trigger = await screen.findByRole("button", {
      name: /购买置顶免费/,
    });
    fireEvent.click(trigger);
    expect(
      await screen.findByRole("button", { name: "24h · 5000 魔力" }),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: /5000 5000/ })).toBe(null);
  });

  it("失败重试复用同一幂等键，成功后才换新键", async () => {
    post.mockRejectedValueOnce(new Error("网络中断"));
    post.mockResolvedValue({});
    mount(true);
    fireEvent.click(
      await screen.findByRole("button", { name: /购买置顶免费/ }),
    );
    const buy = await screen.findByRole("button", { name: "购买" });
    fireEvent.click(buy);
    await screen.findByText(/网络异常/);
    fireEvent.click(buy);
    await waitFor(() => expect(post).toHaveBeenCalledTimes(2));
    const first = post.mock.calls[0][1].idempotency_key;
    const retry = post.mock.calls[1][1].idempotency_key;
    expect(retry).toBe(first);
    fireEvent.click(buy);
    await waitFor(() => expect(post).toHaveBeenCalledTimes(3));
    expect(post.mock.calls[2][1].idempotency_key).not.toBe(first);
  });
});
