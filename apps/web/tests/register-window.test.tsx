import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import RegisterPage from "@/app/(auth)/register/page";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-689 注册页开放轮窗口：
 * ① 预约窗口未开始（后端可能仍报 open，但窗口闸会拒免邀请码请求）
 *    ⇒ 邀请码必填 + 开放预告，不能等用户填完才在提交处吃「尚未开始」。
 * ② 窗口生效（名额/截止）⇒ 顶部额度条如实显示。
 * ③ slots=0（不限名额）⇒ 显示「不限名额」，不能印出 i64::MAX
 *    （旧后端下发 slots_left=9223372036854775807）。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

let MODE = "open";
let WINDOW: Record<string, unknown> | undefined;

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
      mode: MODE,
      captcha_provider: "none",
      reg_window: WINDOW,
    });
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
  WINDOW = undefined;
  get.mockReset();
  get.mockImplementation(route);
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({ json: async () => ({ data: [] }) })),
  );
});

describe("注册页开放轮窗口（P-689）", () => {
  it("预约未开始：邀请码必填 + 开放预告", async () => {
    MODE = "open";
    WINDOW = {
      label: "周年庆",
      closed: true,
      not_started: true,
      open_at: "2026-11-01T08:00:00+08:00",
    };
    mount();
    await waitFor(() =>
      expect(screen.getByTestId("reg-window-soon")).toBeTruthy(),
    );
    expect(
      screen.getByTestId("reg-window-soon").textContent,
    ).toContain("周年庆");
    const invite = screen.getByLabelText(/邀请码/) as HTMLInputElement;
    expect(invite.required).toBe(true);
    expect(screen.queryByText(zhCN.register.inviteOptional)).toBeNull();
  });

  it("窗口生效：额度条显示剩余/总数", async () => {
    WINDOW = {
      label: "开放轮",
      closed: false,
      slots_left: 3,
      slots_total: 5,
    };
    mount();
    const chip = await screen.findByTestId("reg-window");
    expect(chip.textContent).toContain("3/5");
    const invite = screen.getByLabelText(/邀请码/) as HTMLInputElement;
    expect(invite.required).toBe(false);
  });

  it("不限名额：显示不限额文案而不是巨大数字", async () => {
    WINDOW = { label: "开放轮", closed: false };
    mount();
    const chip = await screen.findByTestId("reg-window");
    expect(chip.textContent).toContain(zhCN.register.windowNoLimit);
    expect(chip.textContent).not.toContain("9223372036854775807");
  });
});
