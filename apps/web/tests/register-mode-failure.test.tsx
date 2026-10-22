import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import RegisterPage from "@/app/(auth)/register/page";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-607：/register-mode 探测失败时，页面曾经把 openMode 默认成 false
 * ＝「邀请制」，于是邀请码 input 带 required，开放注册的访客被浏览器
 * 原生校验挡在提交之前；角标还断言「需要邀请码 · 申请邀请码」。
 * 探测失败只说明模式未知，不能替服务端做这个判断。
 *
 * 负控：把初始值改回 useMemo 式 false / catch 里 setOpenMode(false)
 * ⇒ 第一条用例红（邀请码变必填）；把 openMode 判据写死 true ⇒ 第二条
 * （invite_only 正控）红——开放/邀请两档都必须由后端模式决定。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

let MODE = "open";

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
    if (MODE === "fail") return Promise.reject(new Error("boom"));
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

beforeEach(() => {
  MODE = "open";
  get.mockReset();
  get.mockImplementation(route);
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ json: async () => ({ data: [] }) })),
  );
});

describe("注册模式探测（P-607）", () => {
  it("探测失败不把邀请码标成必填，也不显示邀请制角标", async () => {
    MODE = "fail";
    mount();
    await waitFor(() => expect(get).toHaveBeenCalled());
    const invite = screen.getByLabelText(/邀请码/) as HTMLInputElement;
    expect(invite.required).toBe(false);
    // 角标整体文案是「邀请制注册 · 无邀请码？申请邀请码」，按内层链接取
    expect(screen.queryByText(zhCN.register.applyForCode)).toBeNull();
    expect(screen.queryByText(zhCN.register.inviteOptional)).toBeNull();
  });

  it("invite_only 正控：邀请码必填且出现申请角标", async () => {
    MODE = "invite_only";
    mount();
    await waitFor(() => expect(get).toHaveBeenCalled());
    expect(
      await screen.findByText(zhCN.register.applyForCode),
    ).toBeTruthy();
    const invite = screen.getByLabelText(/邀请码/) as HTMLInputElement;
    expect(invite.required).toBe(true);
  });

  it("open 正控：邀请码非必填并提示可留空", async () => {
    mount();
    await waitFor(() =>
      expect(screen.getByText(zhCN.register.inviteOptional)).toBeTruthy(),
    );
    const invite = screen.getByLabelText(/邀请码/) as HTMLInputElement;
    expect(invite.required).toBe(false);
  });
});
