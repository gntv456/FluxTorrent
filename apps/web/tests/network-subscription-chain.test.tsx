import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { zhCN } from "@/i18n/zh-CN";
import { LocaleProvider } from "@/i18n/client";

/** P-588：厂牌/字幕组订阅链接线。
 *  厂牌页此前没有订阅按钮；追更中心也没有这一档，
 *  接口 /me/subscriptions/networks 没有任何前端消费者。 */

vi.mock("next/navigation", () => ({
  notFound: () => {
    throw new Error("notFound");
  },
  useRouter: () => ({ refresh: vi.fn() }),
  redirect: vi.fn(),
}));
vi.mock("@/i18n/server", () => ({
  getDict: async () => ({
    dict: zhCN,
    locale: "zh-CN",
    currency: "魔力",
  }),
}));

import { api } from "@/lib/api-client";
import NetworkDetailPage from "@/app/(main)/networks/[id]/page";
import SubscriptionsPage from "@/app/(main)/me/subscriptions/page";

function wrap(ui: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {ui}
    </LocaleProvider>,
  );
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("厂牌页订阅按钮", () => {
  it("subscribed 由接口带回，按钮直接显示已订阅", async () => {
    const get = vi.spyOn(api, "get").mockResolvedValue({
      id: 24,
      kind: "network",
      name: "Acme",
      subscribed: true,
      items: [],
    });
    wrap(await NetworkDetailPage({ params: Promise.resolve({ id: "24" }) }));
    expect(get).toHaveBeenCalledWith("/api/v1/networks/24");
    expect(screen.getByRole("button").textContent)
      .toContain(zhCN.networks.subscribed);
    expect(screen.getByText(zhCN.networks.emptyItems)).toBeInTheDocument();
  });

  it("未订阅时显示订阅而非已订阅", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: 24,
      kind: "network",
      name: "Acme",
      subscribed: false,
      items: [],
    });
    wrap(await NetworkDetailPage({ params: Promise.resolve({ id: "24" }) }));
    expect(screen.getByRole("button").textContent)
      .toContain(zhCN.networks.subscribe);
  });
});

describe("追更中心的出品方/字幕组档", () => {
  it("两档 kind 都取，行链到各自详情页并带退订", async () => {
    const get = vi.spyOn(api, "get").mockImplementation(
      async (path: string) => {
        if (path === "/api/v1/me/subscriptions/networks?kind=network") {
          return [
            { network_id: 24, name: "Acme", kind: "network", torrents: 3 },
          ];
        }
        if (
          path ===
          "/api/v1/me/subscriptions/networks?kind=subtitle_group"
        ) {
          return [
            {
              network_id: 15,
              name: "Subs",
              kind: "subtitle_group",
              torrents: 7,
            },
          ];
        }
        if (path === "/api/v1/me/subscriptions/groups") return [];
        if (path === "/api/v1/me/subscriptions/calendar") return [];
        throw new Error(`unexpected ${path}`);
      },
    );
    wrap(
      await SubscriptionsPage({
        searchParams: Promise.resolve({ tab: "networks" }),
      }),
    );
    expect(get).toHaveBeenCalledWith(
      "/api/v1/me/subscriptions/networks?kind=network",
    );
    expect(get).toHaveBeenCalledWith(
      "/api/v1/me/subscriptions/networks?kind=subtitle_group",
    );
    expect(screen.getByRole("link", { name: "Acme" }))
      .toHaveAttribute("href", "/networks/24");
    expect(screen.getByRole("link", { name: "Subs" }))
      .toHaveAttribute("href", "/subtitle-groups/15");
    const unsub = zhCN.subscriptions.unsubscribe;
    expect(screen.getAllByRole("button", { name: unsub })).toHaveLength(2);
  });
});
