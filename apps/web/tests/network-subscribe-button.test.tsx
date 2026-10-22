import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SubscribeNetworkButton } from "@/components/subscribe-network-button";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** P-588：厂牌/字幕组订阅按钮初始态来自 /networks/{id} 的 subscribed，
 *  与 P-583 组订阅同口径；缺了它刷新后已订用户仍看到「订阅」。 */

function wrap(ui: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {ui}
    </LocaleProvider>,
  );
}

describe("SubscribeNetworkButton 订阅态", () => {
  it("厂牌页初始已订阅 → 渲染已订阅，点击走退订", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    wrap(
      <SubscribeNetworkButton
        networkId={31}
        initialSubscribed
        scope="network"
      />,
    );
    const btn = screen.getByRole("button");
    expect(btn.textContent).toContain(zhCN.networks.subscribed);
    await userEvent.click(btn);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith(
        "/api/v1/networks/31/unsubscribe",
        {},
      ),
    );
  });

  it("字幕组页初始未订阅 → 点击走订阅", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    wrap(<SubscribeNetworkButton networkId={32} />);
    const btn = screen.getByRole("button");
    expect(btn.textContent).toContain(zhCN.subtitleGroups.subscribe);
    await userEvent.click(btn);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith(
        "/api/v1/networks/32/subscribe",
        {},
      ),
    );
  });
});
