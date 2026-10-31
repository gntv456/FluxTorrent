import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LoginForm } from "@/app/login/login-form";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-568：登录失败的具体原因不能被字典那句「参数校验失败」盖掉。
 *
 * 后端 1002 的 message 是「标签: 具体判据」（errors.rs 统一拼），
 * 旧版 submit 的 catch 优先取 dict.errors[code] ⇒ 停用 / 归档 /
 * 邮箱未激活三类账号只看到「参数校验失败」，下一步指引（申诉入口、
 * 激活页）全部不可见。本用例钉住「后端尾巴必须展示」。
 *
 * 负控：把 submit 的 setError 换回 dict.errors[err.code] ?? err.message
 * ⇒ 本文件必须红（渲染出的 alert 里没有「长期未登录」）。
 */

const { post } = vi.hoisted(() => ({ post: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), refresh: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: {
      post,
      get: vi.fn(() => Promise.resolve({ mode: "invite_only" })),
    },
    setSessionCookie: vi.fn(),
    hasSessionCookie: () => false,
  };
});

vi.mock("@/lib/pow-client", () => ({
  solvePow: vi.fn(async () => ({ token: "t", solution: "1" })),
  powFromErrorData: async () => null,
}));

const DORMANT =
  "参数校验失败: 账号因长期未登录已被停用，请通过『联系我们』" +
  "附上用户名申请恢复";

beforeEach(() => {
  post.mockReset();
});

describe("登录失败原因（P-568）", () => {
  it("1002 的具体判据要露出来，不能只剩「参数校验失败」", async () => {
    post.mockRejectedValueOnce(
      new ApiError(1002, DORMANT, null, 400, "t"),
    );
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <LoginForm />
      </LocaleProvider>,
    );
    await userEvent.type(
      screen.getByPlaceholderText(zhCN.login.username),
      "alice",
    );
    await userEvent.type(
      screen.getByPlaceholderText(zhCN.login.password),
      "pw",
    );
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.login.submit }),
    );
    const alert = await screen.findByRole("alert");
    await waitFor(() =>
      expect(alert.textContent).toContain("长期未登录已被停用"),
    );
    expect(alert.textContent).not.toBe(zhCN.errors[1002]);
  });
});
