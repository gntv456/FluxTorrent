import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { zhCN } from "@/i18n/zh-CN";

/** P-594：/staffbox 的默认「咨询工作台」调 /staffmessages，而它要
 *  staff.message（现网只授 class>=90）⇒ 会员点侧栏「管理组信箱」
 *  先看到一张 403 错误面板。会员应落到 /contactstaff。 */

const { redirect } = vi.hoisted(() => ({
  redirect: vi.fn((_url: string) => {
    throw new Error("NEXT_REDIRECT");
  }),
}));
vi.mock("next/navigation", () => ({ redirect }));
vi.mock("@/i18n/server", () => ({
  getDict: async () => ({
    dict: zhCN,
    locale: "zh-CN",
    currency: "魔力",
  }),
}));
vi.mock("@/components/module-gate", () => ({
  requireModule: async () => null,
}));
vi.mock("@/components/staff-box", () => ({
  StaffBox: () => <div>staff-box-view</div>,
}));

import { api } from "@/lib/api-client";
import StaffBoxPage from "@/app/(main)/staffbox/page";

beforeEach(() => {
  vi.restoreAllMocks();
  redirect.mockClear();
});

describe("管理组信箱按等级分流", () => {
  it("会员（class 1）跳 /contactstaff", async () => {
    vi.spyOn(api, "get").mockResolvedValue({ class_id: 1 } as never);
    await expect(StaffBoxPage()).rejects.toThrow("NEXT_REDIRECT");
    expect(redirect).toHaveBeenCalledWith("/contactstaff");
  });

  it("staff（class 90）照常看到工作台", async () => {
    vi.spyOn(api, "get").mockResolvedValue({ class_id: 90 } as never);
    render(await StaffBoxPage());
    expect(screen.getByText("staff-box-view")).toBeInTheDocument();
    expect(redirect).not.toHaveBeenCalled();
  });

  it("/me 取不到时放行，不误挡 staff", async () => {
    vi.spyOn(api, "get").mockRejectedValue(new Error("down"));
    render(await StaffBoxPage());
    expect(screen.getByText("staff-box-view")).toBeInTheDocument();
    expect(redirect).not.toHaveBeenCalled();
  });
});
