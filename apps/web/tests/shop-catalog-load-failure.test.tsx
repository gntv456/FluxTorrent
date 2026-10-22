import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

/**
 * P-616：GET /shop/items 失败时，getShopItems 曾回落空数组 ⇒ 商店页画出
 * 「全部 0」的空货架，会员会以为商品下架了。现在失败态单独一句话。
 *
 * 负控：回放 HEAD page + lib/data ⇒ 第一条红在「找不到 加载失败」，
 * 快照里空货架 tab 仍在；空货架正控两边都绿。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get } };
});
vi.mock("@/components/module-gate", () => ({
  requireModule: async () => null,
}));
vi.mock("@/i18n/server", async () => {
  const { zhCN } = await import("@/i18n/zh-CN");
  return {
    getDict: async () => ({
      dict: zhCN,
      locale: "zh-CN",
      currency: "魔力",
    }),
  };
});
vi.mock("@/components/voucher-panel", () => ({
  VoucherPanel: () => null,
}));

import ShopPage from "@/app/(main)/shop/page";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

async function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {await ShopPage()}
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
});

describe("商城目录取数失败（P-616）", () => {
  it("失败显示加载失败，不画空货架", async () => {
    get.mockImplementation(async () => {
      throw new ApiError(5000, "服务暂时不可用", null, 500, "t");
    });
    await mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByRole("tab")).toBeNull();
  });

  it("正控：真实空货架仍画目录（全部 0）", async () => {
    get.mockImplementation(async (path: string) => {
      if (path === "/api/v1/shop/items") return [];
      if (path === "/api/v1/me/spark") return { balance: 5 };
      throw new Error(`unexpected ${path}`);
    });
    await mount();
    expect(await screen.findByRole("tab")).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
