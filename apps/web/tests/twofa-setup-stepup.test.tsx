import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TwoFactorSetup } from "@/components/twofa-setup";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-496：签发 2FA 密钥前必须复核密码。
 *
 * 后端已把 setup 改成「密码必对才发密钥」，界面这一侧要同步两件事：
 * ① 生成按钮旁要有密码输入，且请求体带 password；
 * ② 密码错时**不能**再把状态翻成「已启用」——旧 catch 把任何错误都当成
 *    「已启用」的提示，接上新校验后输错密码就会看到「✅ 已启用」。
 */

const S = zhCN.usercp.security;
const T = zhCN.security2fa;
let setupBody: Record<string, unknown> | null = null;
let enableBody: Record<string, unknown> | null = null;

function res(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => "application/json" },
    json: async () => body,
  } as unknown as Response;
}

function stub(api: { setupFail?: boolean }) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith("/me/overview")) {
        return res({ code: 0, data: { totp_enabled: false }, request_id: "t" });
      }
      if (init?.method === "POST" && url.endsWith("/me/2fa/setup")) {
        setupBody = JSON.parse(String(init.body ?? "{}"));
        if (api.setupFail) {
          return res(
            {
              code: 1002,
              message: "参数校验失败: 启用 2FA 未执行：凭证无效",
              data: null,
              request_id: "t",
            },
            400,
          );
        }
        return res({
          code: 0,
          data: { secret: "JBSWY3DPEHPK3PXP", otpauth_uri: "otpauth://totp/x" },
          request_id: "t",
        });
      }
      if (init?.method === "POST" && url.endsWith("/me/2fa/enable")) {
        enableBody = JSON.parse(String(init.body ?? "{}"));
        return res({ code: 0, data: { enabled: true }, request_id: "t" });
      }
      return res({ code: 0, data: null, request_id: "t" });
    }),
  );
}

beforeEach(() => {
  setupBody = null;
  enableBody = null;
  document.cookie = "flux.session=1; path=/";
  document.cookie = "flux_csrf=abc123; path=/";
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <TwoFactorSetup />
    </LocaleProvider>,
  );
}

describe("2FA 自助签发（P-496）", () => {
  it("密码为空时按钮不可点；填密码后 setup 请求体带 password", async () => {
    stub({});
    mount();
    const btn = await screen.findByRole("button", { name: T.setup });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
    await userEvent.type(
      screen.getByPlaceholderText(S.oldPassword),
      "s3cret-pass",
    );
    expect((btn as HTMLButtonElement).disabled).toBe(false);
    await userEvent.click(btn);
    await waitFor(() => expect(setupBody).not.toBeNull());
    expect(setupBody).toEqual({ password: "s3cret-pass" });
    expect(await screen.findByText(/JBSWY3DPEHPK3PXP/)).toBeTruthy();
  });

  it("密码被拒时不把状态翻成「已启用」，并显示后端那句中文", async () => {
    stub({ setupFail: true });
    mount();
    await screen.findByText(/未启用/);
    await userEvent.type(
      screen.getByPlaceholderText(S.oldPassword),
      "wrong-pass",
    );
    await userEvent.click(screen.getByRole("button", { name: T.setup }));
    expect(
      await screen.findByText("启用 2FA 未执行：凭证无效"),
    ).toBeTruthy();
    expect(screen.queryByText(new RegExp(T.statusOn))).toBeNull();
    expect(screen.getByText(/未启用/)).toBeTruthy();
  });

  it("确认启用仍按 6 位码提交整数", async () => {
    stub({});
    mount();
    await userEvent.type(
      screen.getByPlaceholderText(S.oldPassword),
      "s3cret-pass",
    );
    await userEvent.click(await screen.findByRole("button", { name: T.setup }));
    await screen.findByText(/JBSWY3DPEHPK3PXP/);
    await userEvent.type(screen.getByPlaceholderText(T.codeLabel), "012345");
    await userEvent.click(screen.getByRole("button", { name: T.enable }));
    await waitFor(() => expect(enableBody).not.toBeNull());
    expect(enableBody).toEqual({ code: 12345 });
  });
});
