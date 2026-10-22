import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { CheatAppealBox } from "@/components/cheat-appeal-box";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-614：GET /my/cheat-appeals 失败（非 2001）曾被落成空数组 ⇒
 * 「我的作弊申诉」显示「暂无申诉记录」，会员会以为申诉记录没了。
 * 现在失败显示加载失败；真实空列表仍是「暂无」；2001 仍隐藏整块。
 *
 * 负控：回放 HEAD cheat-appeal-box ⇒ 第一条红在「暂无申诉记录仍在」，
 * 后两条正控两边都绿。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post: vi.fn() } };
});

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <CheatAppealBox />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
});

describe("我的作弊申诉取数失败（P-614）", () => {
  it("失败显示加载失败，不再显示暂无申诉记录", async () => {
    get.mockRejectedValueOnce(
      new ApiError(5000, "服务暂时不可用", null, 500, "t"),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.cheaterbox.myAppealEmpty)).toBeNull();
    // 免登录令牌申诉入口不受列表取数影响
    expect(screen.getByText(zhCN.cheaterbox.tokenTitle)).toBeTruthy();
  });

  it("正控：真实空列表仍显示暂无申诉记录", async () => {
    get.mockResolvedValueOnce([]);
    mount();
    expect(
      await screen.findByText(zhCN.cheaterbox.myAppealEmpty),
    ).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });

  it("正控：2001 隐藏申诉列表块，令牌入口保留", async () => {
    get.mockRejectedValueOnce(
      new ApiError(2001, "请先登录", null, 401, "t"),
    );
    mount();
    await waitFor(() =>
      expect(screen.queryByText(zhCN.cheaterbox.myAppealTitle)).toBeNull(),
    );
    expect(screen.getByText(zhCN.cheaterbox.tokenTitle)).toBeTruthy();
  });
});
