import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LoginForm } from "@/app/login/login-form";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-502 登录 PoW 恢复链。
 *
 * 后端在触发线（连续失败 ≥2）要求工作量证明，旧版校验失败回 1002
 * 「参数校验失败」——前端 `err.code === 2012` 的自动取题重提分支永远
 * 不命中；页面一刷新本地 attempts 归零，带正确密码也只会 1002，再试
 * 几次直接被锁 15 分钟。另：PoW 令牌被服务端 GETDEL 一次性消费，
 * 失败后若继续复用，下一次提交注定再失败。
 *
 * 负控：抽掉 login-form 的 `powRef.current = null`，用例②必须红；
 * 删掉 catch 里的 2012 分支，用例①必须红。
 */

const { push, solve, post } = vi.hoisted(() => ({
  push: vi.fn(),
  solve: vi.fn(),
  post: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push, replace: vi.fn(), refresh: vi.fn() }),
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
  solvePow: (...args: unknown[]) => solve(...args),
  powFromErrorData: async () => null,
}));

const OK = {
  token: "jwt",
  must_reset_password: false,
  user: { id: 1, username: "alice", class_id: 1 },
};

function deny(code: number): ApiError {
  return new ApiError(code, "x", null, 400, "t");
}

async function mountAndFill() {
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
  return screen.getByRole("button", { name: zhCN.login.submit });
}

/** 点一次并等这一轮请求落地 + 按钮恢复可点（busy 复位） */
async function submit(
  btn: HTMLElement,
  calls: number,
) {
  await userEvent.click(btn);
  // 2012 会在同一次点击内自动重提，这里只等「至少打了这么多轮」
  await waitFor(() =>
    expect(post.mock.calls.length).toBeGreaterThanOrEqual(calls),
  );
  await waitFor(() => expect(btn).not.toBeDisabled());
}

beforeEach(() => {
  push.mockReset();
  solve.mockReset();
  post.mockReset();
});

describe("登录 PoW（P-502）", () => {
  it("2012 → 自动取题求解并重提", async () => {
    post.mockRejectedValueOnce(deny(2012)).mockResolvedValueOnce(OK);
    solve.mockResolvedValueOnce({ token: "tok9", solution: "42" });
    const btn = await mountAndFill();

    await submit(btn, 1);
    await waitFor(() => expect(post).toHaveBeenCalledTimes(2));

    expect(solve).toHaveBeenCalledTimes(1);
    expect(post.mock.calls[1][1]).toMatchObject({
      pow_token: "tok9",
      pow_solution: "42",
    });
    await waitFor(() => expect(push).toHaveBeenCalledWith("/torrents"));
  });

  it("令牌一次性：失败后下轮必须重新求解", async () => {
    post
      .mockRejectedValueOnce(deny(2004))
      .mockRejectedValueOnce(deny(2004))
      .mockRejectedValueOnce(deny(2004))
      .mockResolvedValueOnce(OK);
    solve
      .mockResolvedValueOnce({ token: "tokA", solution: "1" })
      .mockResolvedValueOnce({ token: "tokB", solution: "2" });
    const btn = await mountAndFill();

    await submit(btn, 1);
    await submit(btn, 2);
    await submit(btn, 3); // 本地 attempts 到 2，开始带 PoW
    await submit(btn, 4);

    expect(solve).toHaveBeenCalledTimes(2);
    expect(post.mock.calls[2][1]).toMatchObject({ pow_token: "tokA" });
    expect(post.mock.calls[3][1]).toMatchObject({ pow_token: "tokB" });
    await waitFor(() => expect(push).toHaveBeenCalledWith("/torrents"));
  });
});
