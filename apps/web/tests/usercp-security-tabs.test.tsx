import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SecurityTab } from "@/components/usercp-security";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";
import type { UserSettings } from "@/components/usercp";

/**
 * 2026-11-12：安全设定由「一张长表竖排四组」改成顶部标签切换。
 * 判据三条：① 默认只渲染凭证那组，别的组行不在 DOM 里（否则「标签切换」
 * 只是视觉折叠，长表照旧把四组全挂上）；② 点标签能换到对应组；③ 账户
 * 概览带 #tokens 跳进来时，自动落到「开放 API 与隐私」那页而不是停在
 * 凭证页——否则锚点目标根本不在 DOM 里，滚到无处可滚。
 */

const { get, post } = vi.hoisted(() => ({ get: vi.fn(), post: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post } };
});

function getRoute(url: string) {
  if (url === "/api/v1/me/overview") {
    return Promise.resolve({ totp_enabled: false });
  }
  if (url === "/api/v1/me/tokens") return Promise.resolve([]);
  if (url === "/api/v1/me/passkeys") return Promise.resolve({ items: [] });
  if (url === "/api/v1/me/logins") return Promise.resolve([]);
  if (url === "/api/v1/me/sessions") return Promise.resolve([]);
  if (url === "/api/v1/me/notice-prefs") return Promise.resolve({});
  return Promise.resolve(null);
}

const S = {
  privacy: "normal",
} as unknown as UserSettings;

beforeEach(() => {
  get.mockReset();
  get.mockImplementation(getRoute);
  post.mockReset();
  post.mockImplementation(() => Promise.resolve({}));
  window.history.replaceState(null, "", "/my?tab=security");
});

const sec = zhCN.usercp.security;

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <SecurityTab s={S} patch={() => {}} />
    </LocaleProvider>,
  );
}

const tabOf = (name: string) =>
  screen.getByRole("tab", { name });

describe("安全设定的分区标签（2026-11-12）", () => {
  it("四个标签齐全，默认停在凭证与密码", () => {
    mount();
    expect(tabOf(sec.grpCredential).getAttribute("aria-selected"))
      .toBe("true");
    for (const k of [sec.grpDevice, sec.grpNotify, sec.grpApi]) {
      expect(tabOf(k).getAttribute("aria-selected")).toBe("false");
    }
    // 默认页有改密入口；别的组不渲染（不是藏起来）
    expect(screen.getByText(sec.confirmPassword)).toBeTruthy();
    expect(screen.queryByText(sec.loginHistory)).toBeNull();
    expect(screen.queryByText(sec.privacy)).toBeNull();
  });

  it("点标签换组：设备 → 通知 → 开放 API，各自只留本组行", async () => {
    mount();

    await userEvent.click(tabOf(sec.grpDevice));
    expect(screen.getByText(sec.loginHistory)).toBeTruthy();
    expect(screen.queryByText(sec.confirmPassword)).toBeNull();

    await userEvent.click(tabOf(sec.grpNotify));
    expect(screen.getByText(sec.tgBind)).toBeTruthy();
    expect(screen.queryByText(sec.loginHistory)).toBeNull();

    await userEvent.click(tabOf(sec.grpApi));
    expect(screen.getByText(sec.privacy)).toBeTruthy();
    expect(screen.queryByText(sec.tgBind)).toBeNull();
  });

  it("#tokens 锚点：自动切到开放 API 页，目标行在 DOM 里", async () => {
    window.history.replaceState(null, "", "/my?tab=security#tokens");
    mount();
    await waitFor(() =>
      expect(tabOf(sec.grpApi).getAttribute("aria-selected"))
        .toBe("true"),
    );
    expect(document.getElementById("tokens")).toBeTruthy();
  });

  it("#passkey 锚点：落在凭证页（默认页），目标行在 DOM 里", async () => {
    window.history.replaceState(null, "", "/my?tab=security#passkey");
    mount();
    await waitFor(() =>
      expect(tabOf(sec.grpCredential).getAttribute("aria-selected"))
        .toBe("true"),
    );
    expect(document.getElementById("passkey")).toBeTruthy();
  });
});
