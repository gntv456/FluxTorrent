import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

import RegisterPage from "@/app/(auth)/register/page";
import { mountCaptchaWidget } from "@/lib/captcha-widget";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-503 注册页：验证码驱动切到 pow 后遗留的 site key。
 *
 * 后台「验证码驱动」下拉含 pow（0372 迁移），而换驱动时旧的
 * captcha_site_key 不会被清空。注册页挂载第三方组件前只挡了
 * none ⇒ `DRIVER_SCRIPT["pow"]` 是 undefined，调用即 TypeError；
 * 该异常落在 register-mode 那条 Promise 链上，被
 * `.catch(() => setOpenMode(false))` 收走 ⇒ 开放注册被演成邀请制：
 * 邀请码 input 重新带 required，没有邀请码的新用户永远提交不了。
 *
 * 负控：去掉 mountCaptchaWidget 对未知驱动的早退，本用例必须红。
 */

const { get, push } = vi.hoisted(() => ({
  get: vi.fn(),
  push: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push, replace: vi.fn(), refresh: vi.fn() }),
}));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: { get, post: vi.fn() },
    setSessionCookie: vi.fn(),
  };
});

/** 只让 register-mode 生效；PoW 取题按失败走，不影响邀请码断言 */
function route(url: string) {
  if (url === "/api/v1/register-mode") {
    return Promise.resolve({
      mode: "open",
      captcha_provider: "pow",
      captcha_site_key: "stale-turnstile-key",
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
  // register-fields 走裸 fetch（页面既有写法）
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ json: async () => ({ data: [] }) })),
  );
});

describe("注册页：驱动切到 pow 后遗留 site key", () => {
  it("开放注册仍不要求邀请码，且 PoW 块正常渲染", async () => {
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <RegisterPage />
      </LocaleProvider>,
    );
    // PoW 块出现＝register-mode 的 then 已跑完（驱动已写进 state）
    await screen.findByText(zhCN.register.powTitle);
    // 等取题失败落地，避免用例结束时还有悬挂的 setState
    await screen.findByText(zhCN.register.powFailed);

    const invite = document.querySelector<HTMLInputElement>(
      "#field-invite_code",
    );
    expect(invite).not.toBeNull();
    expect(invite!.required).toBe(false);
  });

  it("已知驱动仍会真注入组件脚本（正控）", () => {
    mountCaptchaWidget("turnstile", "k1", vi.fn());
    const sc = document.querySelector<HTMLScriptElement>(
      'script[src^="https://challenges.cloudflare.com/turnstile"]',
    );
    expect(sc).not.toBeNull();
    expect(sc!.src).toContain("sitekey=k1");
  });
});
