import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { PasskeysCard } from "@/components/passkeys-card";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-621：Passkey 列表取数失败时，P-314 的 alert 与「你还没有 Passkey」
 * 空态同时出现——面板一边说加载失败，一边断言钥匙被清空。修复只改空态
 * 的显示条件。负控：回放 HEAD ⇒ 第一条红（空态文案仍在）。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

let listOk = true;

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post: vi.fn() } };
});

function route(url: string) {
  if (url === "/api/v1/me/passkeys") {
    return listOk
      ? Promise.resolve({ items: [] })
      : Promise.reject(new Error("boom"));
  }
  if (url === "/api/v1/me/overview") {
    return Promise.resolve({ totp_enabled: false });
  }
  return Promise.reject(new Error(`unexpected ${url}`));
}

beforeEach(() => {
  listOk = true;
  get.mockReset();
  get.mockImplementation(route);
});

const sec = zhCN.usercp.security;

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <PasskeysCard />
    </LocaleProvider>,
  );
}

describe("P-621 Passkey 列表取数失败", () => {
  it("失败只报加载失败，不显示「你还没有 Passkey」", async () => {
    listOk = false;
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(sec.webauthnEmpty)).toBeNull();
  });

  it("真实空列表仍显示空态（正控）", async () => {
    mount();
    expect(await screen.findByText(sec.webauthnEmpty)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
