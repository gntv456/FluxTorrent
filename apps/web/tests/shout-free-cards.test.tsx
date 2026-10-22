import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ShoutFreeCards, type FreeCard } from "../components/shout-free-cards";

const CARD: FreeCard = {
  id: 42,
  name: "星际穿越 IMAX 版",
  small_descr: "2014 4K HDR",
  size: 59441111040,
  seeders: 7,
  leechers: 1,
  poster: null,
  promotion: "free",
};

describe("ShoutFreeCards（/free 播报带种卡）", () => {
  it("免费种子渲染成可点紧凑行卡（名称链接/促销标/大小/做种）", () => {
    render(<ShoutFreeCards cards={[CARD]} />);
    const link = screen.getByRole("link", { name: /星际穿越/ });
    expect(link).toHaveAttribute("href", "/torrent/42");
    // 促销标：free 专属绿色类（与种子列表同款）
    const badge = screen.getByText("免费");
    expect(badge.className).toContain("torrents-promo--free");
    // 大小 + 做种数随行走查（formatBytes 55.4 GB）
    expect(link.textContent).toContain("55.4 GB");
    expect(link.textContent).toContain("做种 7");
  });

  it("x2free 走通用促销标（文案 2x免费，无 --free 专属修饰）", () => {
    render(<ShoutFreeCards cards={[{ ...CARD, promotion: "x2free" }]} />);
    const badge = screen.getByText("2x免费");
    expect(badge.className).toContain("torrents-promo");
    expect(badge.className).not.toContain("torrents-promo--free");
  });

  it("多卡按序渲染，各自链接不串台", () => {
    render(
      <ShoutFreeCards
        cards={[CARD, { ...CARD, id: 43, name: "沙丘 2021" }]}
      />,
    );
    expect(screen.getByRole("link", { name: /星际穿越/ })).toHaveAttribute(
      "href",
      "/torrent/42",
    );
    expect(screen.getByRole("link", { name: /沙丘/ })).toHaveAttribute(
      "href",
      "/torrent/43",
    );
  });

  // P-551：促销的「免费」只免上传量扣费，不免种子定价（charge_for_download
  // 从不看促销）。播报把有价种说成免费却不给价格，会员点进去才被扣魔力。
  it("有价种在播报卡上带定价（复用下载口同款价格标）", () => {
    const { container } = render(
      <ShoutFreeCards cards={[{ ...CARD, id: 45, price: 800 }]} />,
    );
    const price = container.querySelector(".td-dl-price");
    expect(price).not.toBeNull();
    expect(price?.textContent).toContain("800");
  });

  it("定价为零时不渲染价格标（免价种不许看着像要收费）", () => {
    const { container } = render(
      <ShoutFreeCards cards={[{ ...CARD, id: 46, price: 0 }]} />,
    );
    expect(container.querySelector(".td-dl-price")).toBeNull();
  });
});
