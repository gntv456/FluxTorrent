import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { GroupSubscribeButton } from "@/components/group-subscribe-button";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** P-583：按钮初始态来自组详情口（initialSubscribed），不是恒 false。 */

function wrap(ui: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {ui}
    </LocaleProvider>,
  );
}

describe("GroupSubscribeButton 订阅态", () => {
  it("初始已订阅 → 渲染已订阅，点击走退订", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    wrap(<GroupSubscribeButton groupId={7} initialSubscribed />);
    const btn = screen.getByRole("button");
    expect(btn.textContent).toContain(zhCN.groupsub.subscribed);
    await userEvent.click(btn);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith(
        "/api/v1/torrents/groups/7/unsubscribe",
        {},
      ),
    );
  });

  it("初始未订阅（默认） → 点击走订阅", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    wrap(<GroupSubscribeButton groupId={8} />);
    const btn = screen.getByRole("button");
    expect(btn.textContent).toContain(zhCN.groupsub.subscribe);
    await userEvent.click(btn);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith(
        "/api/v1/torrents/groups/8/subscribe",
        {},
      ),
    );
  });
});
