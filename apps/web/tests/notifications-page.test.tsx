import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { zhCN } from "@/i18n/zh-CN";

/** P-590：/site-activity 回数组，通知中心却按 {data: []} 取，
 *  「全站动态」卡在任何情况下都不出现；且卡原链接 / 没有动态流。 */

vi.mock("@/i18n/server", () => ({
  getDict: async () => ({
    dict: zhCN,
    locale: "zh-CN",
    currency: "魔力",
  }),
}));

import { api } from "@/lib/api-client";
import NotificationsPage from "@/app/(main)/notifications/page";

const acts = [
  {
    id: 60,
    kind: "award_reveal",
    text: "「测试」期评选已揭晓",
    created_at: new Date().toISOString(),
  },
  {
    id: 59,
    kind: "subtitle_upload",
    text: "bic 上传了字幕「测试字幕」",
    created_at: new Date().toISOString(),
  },
];

function mockApi(activity: unknown | Error) {
  return vi.spyOn(api, "get").mockImplementation(async (path: string) => {
    if (path === "/api/v1/me/overview") return { unread_messages: 3 };
    if (path === "/api/v1/me/hr") return { rows: [] };
    if (path === "/api/v1/messages/staff") return [];
    if (path === "/api/v1/site-activity") {
      if (activity instanceof Error) throw activity;
      return activity;
    }
    throw new Error(`unexpected ${path}`);
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("通知中心的全站动态", () => {
  it("数组响应能出计数卡，卡下方列出动态正文", async () => {
    const get = mockApi(acts);
    render(await NotificationsPage());
    expect(get).toHaveBeenCalledWith("/api/v1/site-activity");
    const cards = screen.getAllByText(zhCN.notifications.activityCard);
    expect(cards.length).toBe(2); // 计数卡 + 列表标题
    expect(screen.getByRole("link", { name: /全站动态/ }))
      .toHaveAttribute("href", "#site-activity");
    expect(screen.getByText("「测试」期评选已揭晓")).toBeInTheDocument();
    expect(screen.getByText("bic 上传了字幕「测试字幕」"))
      .toBeInTheDocument();
  });

  it("动态流取数失败时不演空卡", async () => {
    mockApi(new Error("down"));
    render(await NotificationsPage());
    expect(screen.queryByText(zhCN.notifications.activityCard))
      .not.toBeInTheDocument();
  });
});
