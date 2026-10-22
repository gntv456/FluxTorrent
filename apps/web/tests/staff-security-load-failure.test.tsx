import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { StaffSecurityPanel } from "@/components/staff-tools-security";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-619：后台安全域三张表（IP 封禁 / 群发记录 / 邮箱黑白名单）在取数
 * 失败时都走 `.catch(() => setX([]))`，界面回「暂无…」——复核人员会
 * 以为现网零封禁、零群发、零规则。负控回放 HEAD 的两份文件 ⇒ 第一条
 * 用例红在「找不到加载失败，反而看到暂无封禁」。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: { get, post: vi.fn(), del: vi.fn() },
  };
});

const flash = vi.fn();

function mount(tab: "bans" | "mail" | "emailbans") {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <StaffSecurityPanel tab={tab} flash={flash} />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
  flash.mockReset();
});

describe("P-619 安全域列表取数失败", () => {
  it("IP 封禁失败不演成「暂无封禁记录」", async () => {
    get.mockRejectedValue(new Error("boom"));
    mount("bans");
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.stafftools.banEmpty)).toBeNull();
  });

  it("IP 封禁真实空列表仍显示空态（正控）", async () => {
    get.mockResolvedValue([]);
    mount("bans");
    expect(await screen.findByText(zhCN.stafftools.banEmpty)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });

  it("群发记录失败不演成「暂无发送记录」", async () => {
    get.mockRejectedValue(new Error("boom"));
    mount("mail");
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.stafftools.mailEmpty)).toBeNull();
  });

  it("邮箱名单失败不演成「暂无邮箱规则」", async () => {
    get.mockRejectedValue(new Error("boom"));
    mount("emailbans");
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.stafftools.ebEmpty)).toBeNull();
  });
});
