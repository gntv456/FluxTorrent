import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { HomeLayoutEditor } from "@/components/home-layout-editor";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-612：/api/v1/home 取数失败曾被落成「空排版」——编辑器画出一张没有
 * 板块的空表格，而本页「空列表保存」= 恢复默认排版，管理员顺手点保存就会
 * 把线上排版清掉。现在失败走 loadFailed + 重试，不暴露可保存的空表格。
 *
 * 负控：回放 HEAD home-layout-editor ⇒ 第一条红（没有加载失败文案、
 * 「保存排版」仍在）；后两条正控两边都绿。
 */

const { get, put } = vi.hoisted(() => ({ get: vi.fn(), put: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, put } };
});

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <HomeLayoutEditor />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
  put.mockReset();
});

describe("首页排版编辑器取数失败（P-612）", () => {
  it("失败显示加载失败，不画可保存的空排版", async () => {
    get.mockRejectedValueOnce(
      new ApiError(5000, "服务暂时不可用", null, 500, "t"),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.homeLayout.save)).toBeNull();
    expect(screen.queryByText(zhCN.homeLayout.hint)).toBeNull();
  });

  it("失败后点重试：取数成功即恢复编辑器", async () => {
    get.mockRejectedValueOnce(
      new ApiError(5000, "服务暂时不可用", null, 500, "t"),
    );
    const user = userEvent.setup();
    mount();
    await screen.findByText(zhCN.common.loadFailed);
    get.mockResolvedValueOnce({
      home_sections: [{ key: "latest", span: 3, in_default: true }],
      home_layout: "",
    });
    await user.click(screen.getByText(zhCN.common.retry));
    expect(await screen.findByText(zhCN.homeLayout.save)).toBeTruthy();
  });

  it("正控：成功按线上配置渲染，保存提交当前排版", async () => {
    get.mockResolvedValueOnce({
      home_sections: [
        { key: "latest", span: 3, in_default: true },
        { key: "news", span: 3, in_default: true },
      ],
      home_layout: JSON.stringify([{ key: "news", span: 1 }]),
    });
    put.mockResolvedValueOnce({});
    const user = userEvent.setup();
    mount();
    await screen.findByText(zhCN.homeLayout.save);
    // 线上配置只有 news（span=1），不是默认的 latest 优先
    const rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(1);
    expect(rows[0].textContent).toContain(zhCN.homeLayout.names.news);
    await user.click(screen.getByText(zhCN.homeLayout.save));
    await waitFor(() => expect(put).toHaveBeenCalled());
    expect(put).toHaveBeenCalledWith("/api/v1/admin/home-layout", [
      { key: "news", span: 1 },
    ]);
  });
});
