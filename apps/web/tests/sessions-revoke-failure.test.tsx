import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SessionsCard } from "@/components/sessions-card";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-609：踢出其它会话失败时，面板复用「未启用」（安全页 2FA 文案）并以
 * 成功色显示——用户看到绿色的「未启用」，既不知道会话没踢掉，也以为
 * 这个功能本身没开。同一处 danger 弹窗还默认把确认钮写成「删除」。
 *
 * 负控：回放 HEAD sessions-card ⇒ 第一条红在「未启用」仍在、失败原因
 * 取不到；第二条红在确认钮文案（「删除」）。
 */

const { get, post } = vi.hoisted(() => ({ get: vi.fn(), post: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post } };
});

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <SessionsCard />
    </LocaleProvider>,
  );
}

/** 打开确认框并返回它；确认钮按「弹窗里最后一个按钮」取，对旧版也成立 */
async function openConfirm() {
  await screen.findByText("1.2.3.4");
  await userEvent.click(
    screen.getByRole("button", { name: zhCN.sessions.revokeBtn }),
  );
  return screen.getByRole("alertdialog");
}

async function confirm(dialog: HTMLElement) {
  const btns = within(dialog).getAllByRole("button");
  await userEvent.click(btns[btns.length - 1]);
}

beforeEach(() => {
  get.mockReset();
  get.mockResolvedValue([
    {
      ip: "1.2.3.4",
      user_agent: "UA",
      last_at: "2026-10-20T00:00:00Z",
      logins: 2,
    },
  ]);
  post.mockReset();
});

describe("会话撤销失败（P-609）", () => {
  it("失败显示真实原因与错误色，不再显示「未启用」", async () => {
    post.mockRejectedValueOnce(
      new ApiError(9999, "服务繁忙，请稍后再试", null, 500, "t"),
    );
    mount();
    await confirm(await openConfirm());
    expect(screen.queryByText(zhCN.security2fa.statusOff)).toBeNull();
    const el = await screen.findByText("服务繁忙，请稍后再试");
    expect(el.className).toContain("text-coral");
  });

  it("正控：确认钮写的是这次动作，成功后显示已踢出", async () => {
    post.mockResolvedValueOnce({});
    mount();
    const dialog = await openConfirm();
    expect(
      within(dialog).getByRole("button", {
        name: zhCN.sessions.revokeBtn,
      }),
    ).toBeTruthy();
    expect(
      within(dialog).queryByRole("button", {
        name: zhCN.ui.confirmDelete,
      }),
    ).toBeNull();
    await confirm(dialog);
    await waitFor(() =>
      expect(screen.getByText(zhCN.sessions.revokedOk)).toBeTruthy(),
    );
  });
});
