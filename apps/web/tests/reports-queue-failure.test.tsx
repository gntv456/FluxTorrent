import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { ReportBox } from "@/components/report-box";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-610：/reports 的管理组队列取数失败时，旧代码只把 rows 落成空数组
 * ⇒ 显示「队列为空」，管理组会以为没人举报（与 P-314 同一形状）。
 * 403/2003 仍是「无权限」，真实空队列仍是「队列为空」。
 *
 * 负控：回放 HEAD report-box ⇒ 第一条红（失败文案被「队列为空」顶替），
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
      <ReportBox />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
});

describe("举报队列取数失败（P-610）", () => {
  it("失败显示加载失败，不再显示队列为空", async () => {
    get.mockRejectedValueOnce(
      new ApiError(5000, "服务暂时不可用", null, 500, "t"),
    );
    mount();
    await waitFor(() =>
      expect(
        screen.queryByText(zhCN.common.loadFailed) ??
          screen.queryByText(zhCN.reportbox.queueEmpty),
      ).not.toBeNull(),
    );
    expect(screen.queryByText(zhCN.reportbox.queueEmpty)).toBeNull();
    expect(screen.getByText(zhCN.common.loadFailed)).toBeTruthy();
  });

  it("正控：真实空队列仍显示队列为空", async () => {
    get.mockResolvedValueOnce([]);
    mount();
    await waitFor(() =>
      expect(screen.getByText(zhCN.reportbox.queueEmpty)).toBeTruthy(),
    );
  });

  it("正控：403/2003 仍显示无权限", async () => {
    get.mockRejectedValueOnce(
      new ApiError(2003, "无权限", null, 403, "t"),
    );
    mount();
    expect(await screen.findByText(zhCN.reportbox.noPerm)).toBeTruthy();
  });
});
