import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { zhCN } from "@/i18n/zh-CN";
import { LocaleProvider } from "@/i18n/client";

/** P-591：接受/拉黑成功后上半部 RSC 列表不刷新——
 *  对方从「收到的申请」消失，却不出现在「好友」区。 */

const { refresh } = vi.hoisted(() => ({ refresh: vi.fn() }));
vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh }),
}));

import { api } from "@/lib/api-client";
import { FriendsActions } from "@/components/friends-actions";

function wrap(ui: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {ui}
    </LocaleProvider>,
  );
}

beforeEach(() => {
  vi.restoreAllMocks();
  refresh.mockClear();
});

describe("好友操作面板刷新 RSC 列表", () => {
  it("接受申请后调用 router.refresh", async () => {
    vi.spyOn(api, "get").mockResolvedValue([
      { username: "bob", list: "incoming" },
    ]);
    const post = vi.spyOn(api, "post").mockResolvedValue({});
    wrap(<FriendsActions />);
    expect(await screen.findByText("bob")).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", {
        name: zhCN.friendsActions.accept,
      }),
    );
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/v1/friends/action", {
        username: "bob",
        action: "accept",
      }),
    );
    expect(refresh).toHaveBeenCalled();
  });

  it("拉黑后调用 router.refresh", async () => {
    vi.spyOn(api, "get").mockResolvedValue([]);
    const post = vi.spyOn(api, "post").mockResolvedValue({});
    wrap(<FriendsActions />);
    const inputs = screen.getAllByRole("textbox");
    await userEvent.type(inputs[1]!, "mallory");
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.friendsActions.blackBtn }),
    );
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/v1/friends/action", {
        username: "mallory",
        action: "black",
      }),
    );
    expect(refresh).toHaveBeenCalled();
  });
});
