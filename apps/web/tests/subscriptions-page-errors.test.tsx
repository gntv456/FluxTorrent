import { describe, it, expect, vi, beforeEach } from "vitest";
import { zhCN } from "@/i18n/zh-CN";

/** P-589：追更中心此前把任何取数失败都 redirect 去登录页。
 *  已登录用户遇到 500 会被反复踢去登录，且故障被演成「没有订阅」。 */

const { redirect } = vi.hoisted(() => ({
  redirect: vi.fn((_url: string) => {
    throw new Error("NEXT_REDIRECT");
  }),
}));
vi.mock("next/navigation", () => ({ redirect }));
vi.mock("@/i18n/server", () => ({
  getDict: async () => ({
    dict: zhCN,
    locale: "zh-CN",
    currency: "魔力",
  }),
}));

import { api, ApiError } from "@/lib/api-client";
import SubscriptionsPage from "@/app/(main)/me/subscriptions/page";

const params = { searchParams: Promise.resolve({}) };

beforeEach(() => {
  vi.restoreAllMocks();
  redirect.mockClear();
});

describe("追更中心取数失败的分诊", () => {
  it("401 才跳登录（带回跳地址）", async () => {
    vi.spyOn(api, "get").mockRejectedValue(
      new ApiError(2001, "会话已失效", undefined, 401),
    );
    await expect(SubscriptionsPage(params)).rejects.toThrow("NEXT_REDIRECT");
    expect(redirect).toHaveBeenCalledWith(
      "/login?next=/me/subscriptions",
    );
  });

  it("500 原样抛给错误边界，不跳登录", async () => {
    const boom = new ApiError(1000, "服务异常", undefined, 500);
    vi.spyOn(api, "get").mockRejectedValue(boom);
    await expect(SubscriptionsPage(params)).rejects.toBe(boom);
    expect(redirect).not.toHaveBeenCalled();
  });
});
