import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import RegisterPage from "@/app/(auth)/register/page";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-567：email_verify（开放注册 + 邮件激活）在前端的三条断链。
 *
 * ① 页面把模式折成「open 才算开放」⇒ email_verify 下邀请码带 required，
 *    浏览器直接挡住提交；后端的 email_verify 分支本来就不需要邀请码。
 * ② 注册成功后无条件 POST /auth/login——email_verify 的新号必然被
 *    guard_login 拦下，用户看到「登录失败」却不知道要先去收邮件。
 *    正确落点是 /email-verify（带 email 预填，方便重发）。
 * ③ 注册失败时 1002 的具体判据被字典标签盖掉（与 P-568 同一形状）。
 *
 * 负控：把 openMode 判据改回 mode==="open"、去掉 verify 分流、把
 * mapError 换回 dict.errors[code] ?? message ⇒ 三条用例各自红。
 */

const { get, post, push } = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  push: vi.fn(),
}));

let MODE = "email_verify";

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push, replace: vi.fn(), refresh: vi.fn() }),
}));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: { get, post },
    setSessionCookie: vi.fn(),
  };
});

function route(url: string) {
  if (url === "/api/v1/register-mode") {
    return Promise.resolve({ mode: MODE, captcha_provider: "none" });
  }
  if (url === "/api/v1/auth/captcha") {
    return Promise.resolve({ captcha_id: "c1", question: "1+1" });
  }
  return Promise.reject(new Error(`unexpected GET ${url}`));
}

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <RegisterPage />
    </LocaleProvider>,
  );
}

async function fill() {
  await userEvent.type(screen.getByLabelText(/用户名/), "alice");
  await userEvent.type(screen.getByLabelText(/邮箱/), "a@b.co");
  await userEvent.type(screen.getByLabelText(/密码/), "password1");
  // 算术验证码（provider=none）答案框带 required
  await userEvent.type(
    screen.getByPlaceholderText(zhCN.register.captchaAnswer),
    "2",
  );
}

beforeEach(() => {
  MODE = "email_verify";
  get.mockReset();
  get.mockImplementation(route);
  post.mockReset();
  push.mockReset();
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ json: async () => ({ data: [] }) })),
  );
});

describe("email_verify 注册页（P-567）", () => {
  it("邀请码非必填，注册成功后落到激活页而不是硬登", async () => {
    post.mockResolvedValueOnce({ user_id: 9 });
    mount();
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/api/v1/register-mode"),
    );
    const invite = screen.getByLabelText(/邀请码/) as HTMLInputElement;
    expect(invite.required).toBe(false);
    expect(screen.queryByText(zhCN.register.inviteBadge)).toBeNull();

    await fill();
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.register.submit }),
    );
    await waitFor(() => expect(post).toHaveBeenCalledTimes(1));
    expect(push).toHaveBeenCalledWith(
      "/email-verify?email=a%40b.co",
    );
  });

  it("注册失败时 1002 的具体判据要露出来", async () => {
    MODE = "open";
    post.mockRejectedValueOnce(
      new ApiError(
        1002,
        "参数校验失败: 该邮箱地址已被禁用，请联系管理组",
        null,
        400,
        "t",
      ),
    );
    mount();
    await fill();
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.register.submit }),
    );
    expect(
      (await screen.findAllByText(/该邮箱地址已被禁用/)).length,
    ).toBeGreaterThan(0);
  });
});
