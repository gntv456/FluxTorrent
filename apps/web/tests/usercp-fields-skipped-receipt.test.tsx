import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { UserFieldsTab } from "@/components/usercp-fields";
import { zhCN } from "@/i18n/zh-CN";

const { get, put } = vi.hoisted(() => ({
  get: vi.fn(),
  put: vi.fn(),
}));

vi.mock("@/lib/api-client", () => ({
  api: {
    get: (...args: unknown[]) => get(...args),
    put: (...args: unknown[]) => put(...args),
  },
}));

beforeEach(() => {
  get.mockReset();
  put.mockReset();
  get.mockResolvedValue([
    {
      key: "qq",
      label: "QQ",
      type: "text",
      required: false,
      options: [],
      value: null,
    },
  ]);
});

async function clickSave() {
  render(<UserFieldsTab />);
  const btn = await screen.findByRole("button", {
    name: zhCN.usercp.saveBtn,
  });
  fireEvent.click(btn);
  return btn;
}

describe("P-572 usercp 自定义字段保存回执", () => {
  it("skipped>0 时报出未保存条数，不许再说全部已保存", async () => {
    put.mockResolvedValue({ saved: 2, skipped: 1 });
    await clickSave();
    expect(
      await screen.findByText("已保存 2 项；1 项字段已下线，未保存"),
    ).toBeTruthy();
    expect(screen.queryByText(zhCN.usercp.fields.saved)).toBeNull();
  });

  it("全部落库时仍报已保存（正控）", async () => {
    put.mockResolvedValue({ saved: 1, skipped: 0 });
    await clickSave();
    expect(await screen.findByText(zhCN.usercp.fields.saved)).toBeTruthy();
  });
});
