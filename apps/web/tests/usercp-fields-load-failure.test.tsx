import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { UserFieldsTab } from "@/components/usercp-fields";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-617：/me 自定义字段取数失败曾走 .catch(() => setDefs([]))，页签把
 * 故障演成「暂无可填写字段」——会员会以为自己站点的自定义字段被删光。
 * 负控：把 catch 改回 setDefs([])（不做 failed 标记）⇒ 第一条用例红。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("@/lib/api-client", () => ({
  api: {
    get: (...args: unknown[]) => get(...args),
    put: vi.fn(),
  },
}));

const FIELD = {
  key: "qq",
  label: "QQ",
  type: "text",
  required: false,
  options: [],
  value: null,
};

beforeEach(() => {
  get.mockReset();
});

describe("P-617 /me 自定义字段取数失败", () => {
  it("失败不演成空态：给出加载失败与重试，不显示「暂无」", async () => {
    get.mockRejectedValueOnce(new Error("boom"));
    render(<UserFieldsTab />);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain(zhCN.common.loadFailed);
    expect(screen.queryByText(zhCN.usercp.fields.empty)).toBeNull();
    expect(
      screen.getByRole("button", { name: zhCN.common.retry }),
    ).toBeTruthy();
  });

  it("真实空列表仍显示空态（正控）", async () => {
    get.mockResolvedValueOnce([]);
    render(<UserFieldsTab />);
    expect(await screen.findByText(zhCN.usercp.fields.empty)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });

  it("重试成功后可填写字段（重试真重新取数）", async () => {
    get.mockRejectedValueOnce(new Error("boom")).mockResolvedValueOnce([
      FIELD,
    ]);
    render(<UserFieldsTab />);
    fireEvent.click(
      await screen.findByRole("button", { name: zhCN.common.retry }),
    );
    expect(await screen.findByText(/QQ/)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
