import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import RegisterPage from "@/app/(auth)/register/page";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-504 注册页：第三方验证码 token 一次性，失败后必须重挂组件。
 *
 * turnstile/recaptcha/hcaptcha 的 token 在 siteverify 端一次性消费；
 * 旧版失败分支只 refreshCaptcha() + 递增 gateNonce，而 gateNonce 只
 * key 了 PoW/答题块——验证码组件不重挂、captchaToken 也不清 ⇒ 此后
 * 每次提交都复用已消费的 token，注册重试永远失败，只能整页刷新。
 *
 * 负控：去掉 RegisterCaptcha 的 key 与重挂 effect，本用例必须红。
 */

const { get, post, mount } = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  mount: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), refresh: vi.fn() }),
}));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: { get, post },
    setSessionCookie: vi.fn(),
  };
});

vi.mock("@/lib/captcha-widget", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("@/lib/captcha-widget")>();
  return {
    ...actual,
    mountCaptchaWidget: (...args: unknown[]) => mount(...args),
  };
});

function route(url: string) {
  if (url === "/api/v1/register-mode") {
    return Promise.resolve({
      mode: "open",
      captcha_provider: "turnstile",
      captcha_site_key: "site-key",
    });
  }
  if (url === "/api/v1/auth/captcha") {
    return Promise.resolve({ captcha_id: "c1", question: "1+1" });
  }
  return Promise.reject(new Error(`unexpected GET ${url}`));
}

async function submitOnly() {
  await userEvent.click(
    screen.getByRole("button", { name: zhCN.register.submit }),
  );
}

async function fillAndSubmit() {
  await userEvent.type(screen.getByLabelText(/用户名/), "alice");
  await userEvent.type(screen.getByLabelText(/邮箱/), "a@b.co");
  await userEvent.type(screen.getByLabelText(/密码/), "password1");
  await submitOnly();
}

beforeEach(() => {
  get.mockReset();
  get.mockImplementation(route);
  post.mockReset();
  post.mockRejectedValue(
    new ApiError(2007, "用户名已被占用", null, 400, "t"),
  );
  mount.mockReset();
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ json: async () => ({ data: [] }) })),
  );
});

describe("注册页：第三方验证码失败后重试", () => {
  it("失败后重挂组件，下一次提交用新 token", async () => {
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <RegisterPage />
      </LocaleProvider>,
    );
    // 首次挂载（register-mode 落地后的 effect）
    await waitFor(() => expect(mount).toHaveBeenCalledTimes(1));
    const firstBox = document.getElementById("cf-turnstile");
    expect(firstBox).not.toBeNull();
    const onFirstToken = mount.mock.calls[0][2] as (t: string) => void;
    act(() => onFirstToken("tok1"));

    await fillAndSubmit();
    await waitFor(() => expect(post).toHaveBeenCalledTimes(1));
    expect(post.mock.calls[0][1]).toMatchObject({
      captcha_token: "tok1",
    });

    // 失败路径确实落地（字段错误渲染）后再看是否重挂
    expect(
      (await screen.findAllByText("用户名已被占用")).length,
    ).toBeGreaterThan(0);
    // 旧 token 已被后端消费：失败后必须重挂组件拿新 token
    await waitFor(() => expect(mount).toHaveBeenCalledTimes(2));
    const secondBox = document.getElementById("cf-turnstile");
    // 同一容器重复 render 会被组件库拒；必须是新节点
    expect(secondBox).not.toBe(firstBox);
    const onSecondToken = mount.mock.calls[1][2] as (t: string) => void;
    act(() => onSecondToken("tok2"));

    await submitOnly();
    await waitFor(() => expect(post).toHaveBeenCalledTimes(2));
    expect(post.mock.calls[1][1]).toMatchObject({
      captcha_token: "tok2",
    });
  });
});
