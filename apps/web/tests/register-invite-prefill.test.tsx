import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import RegisterPage from "@/app/(auth)/register/page";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 邀请邮件里的链接是 /register?invite=<code>。落地页必须把码预填进
 * 邀请码输入框，否则收件人还要从长链接里手抄 32 位码——邮件邀请链
 * 在最后一步断掉。预填后还应出现「定向邀请须用收件邮箱」的提示。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), refresh: vi.fn() }),
}));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: { get, post: vi.fn() },
    setSessionCookie: vi.fn(),
  };
});

function route(url: string) {
  if (url === "/api/v1/register-mode") {
    return Promise.resolve({
      mode: "invite_only",
      captcha_provider: "none",
    });
  }
  if (url === "/api/v1/auth/captcha") {
    return Promise.resolve({ captcha_id: "c1", question: "1+1" });
  }
  return Promise.reject(new Error(`unexpected GET ${url}`));
}

beforeEach(() => {
  get.mockReset();
  get.mockImplementation(route);
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ json: async () => ({ data: [] }) })),
  );
  window.history.pushState({}, "", "/register?invite=INVITE7F3A");
});

describe("邀请邮件落地（?invite=）", () => {
  it("邀请码自动预填并提示定向邀请", async () => {
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <RegisterPage />
      </LocaleProvider>,
    );
    const invite = screen.getByLabelText(/邀请码/) as HTMLInputElement;
    await waitFor(() => expect(invite.value).toBe("INVITE7F3A"));
    expect(screen.getByText(zhCN.register.emailMustMatch)).toBeTruthy();
  });
});
