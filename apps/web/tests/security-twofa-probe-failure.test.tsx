import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ApiTokens } from "@/components/api-tokens";
import { PasskeysCard } from "@/components/passkeys-card";
import { TwoFactorSetup } from "@/components/twofa-setup";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-608：安全面板三张卡都拿 /me/overview 的 totp_enabled 决定「要不要收
 * 当前验证码」。探测一旦失败，旧代码把 twofa 落成 false：
 *  · 签发 API Token / 绑定 Passkey 的验证码输入框整个消失，而服务端
 *    step_up 对已开 2FA 的账号仍要求验证码 ⇒ 错误文案指向一个界面上
 *    不存在的字段，用户卡死；
 *  · 2FA 卡片的启用态永远停在「查询中…」。
 *
 * 负控：回放 HEAD 三个组件 ⇒ 用例 1/3 取不到验证码输入框、用例 4 取不到
 * 「加载失败」，各自红；用例 2 是「已启用仍强制 6 位」的正控，两边都绿。
 */

const { get, post } = vi.hoisted(() => ({ get: vi.fn(), post: vi.fn() }));

let OVERVIEW: "fail" | "on" | "off" = "fail";

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post } };
});

function getRoute(url: string) {
  if (url === "/api/v1/me/overview") {
    if (OVERVIEW === "fail") return Promise.reject(new Error("boom"));
    return Promise.resolve({ totp_enabled: OVERVIEW === "on" });
  }
  if (url === "/api/v1/me/tokens") return Promise.resolve([]);
  if (url === "/api/v1/me/passkeys") return Promise.resolve({ items: [] });
  return Promise.reject(new Error(`unexpected GET ${url}`));
}

function postRoute(url: string) {
  if (url === "/api/v1/me/tokens") return Promise.resolve({ token: "t1" });
  if (url === "/api/v1/auth/passkeys/begin") {
    return Promise.resolve({ challenge: "c", rp_id: "localhost", user_id: 7 });
  }
  return Promise.resolve({});
}

function mount(node: React.ReactNode) {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {node}
    </LocaleProvider>,
  );
}

const sec = zhCN.usercp.security;

beforeEach(() => {
  OVERVIEW = "fail";
  get.mockReset();
  get.mockImplementation(getRoute);
  post.mockReset();
  post.mockImplementation(postRoute);
});

describe("2FA 状态探测失败（P-608）", () => {
  it("API 令牌：验证码框仍在，且填了会随签发请求提交", async () => {
    mount(<ApiTokens />);
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/api/v1/me/overview"),
    );
    const code = screen.getByPlaceholderText(sec.webauthnCodePh);
    await userEvent.type(
      screen.getByPlaceholderText(zhCN.apitokens.namePlaceholder),
      "rss",
    );
    await userEvent.type(screen.getByLabelText(sec.webauthnPwPh), "pw");
    await userEvent.type(code, "123456");
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.apitokens.issue }),
    );
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/v1/me/tokens", {
        name: "rss",
        password: "pw",
        totp: 123456,
      }),
    );
  });

  it("已开 2FA 正控：6 位未填满时签发钮仍禁用", async () => {
    OVERVIEW = "on";
    mount(<ApiTokens />);
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/api/v1/me/overview"),
    );
    const btn = screen.getByRole("button", { name: zhCN.apitokens.issue });
    await userEvent.type(
      screen.getByPlaceholderText(zhCN.apitokens.namePlaceholder),
      "rss",
    );
    await userEvent.type(screen.getByLabelText(sec.webauthnPwPh), "pw");
    expect(btn).toBeDisabled();
    await userEvent.type(
      screen.getByPlaceholderText(sec.webauthnCodePh),
      "123456",
    );
    expect(btn).toBeEnabled();
  });

  it("Passkey：验证码框仍在，并随 begin 复核提交", async () => {
    mount(<PasskeysCard />);
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/api/v1/me/passkeys"),
    );
    await userEvent.click(
      screen.getByRole("button", { name: sec.webauthnBind }),
    );
    const code = screen.getByPlaceholderText(sec.webauthnCodePh);
    await userEvent.type(
      screen.getByPlaceholderText(
        new RegExp(sec.webauthnPwPh),
      ),
      "pw",
    );
    await userEvent.type(code, "123456");
    await userEvent.click(
      screen.getByRole("button", { name: sec.webauthnStepUpBtn }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.ui.confirmOk }),
    );
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/v1/auth/passkeys/begin", {
        label: sec.webauthnNameFallback,
        password: "pw",
        totp: 123456,
      }),
    );
  });

  it("2FA 卡片：探测失败显示加载失败，而不是永远的「查询中」", async () => {
    mount(<TwoFactorSetup />);
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/api/v1/me/overview"),
    );
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.security2fa.statusLoading)).toBeNull();
  });
});
