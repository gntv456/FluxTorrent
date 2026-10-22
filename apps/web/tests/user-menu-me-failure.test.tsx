import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import { zhCN } from "@/i18n/zh-CN";
import { LocaleProvider } from "@/i18n/client";

/**
 * P-611：顶栏 UserMenu 对 /me 的任何失败都清 flux.session 标记与
 * localStorage 里的用户摘要——一次网络抖动/500 就把已登录用户演成游客，
 * 客户端各处 hasSessionCookie() 也跟着失效（2001 本来由 api-client
 * 统一跳登录）。失败只能标记未知：不清凭证、不画「登录」入口。
 *
 * 负控：回放 HEAD user-menu ⇒ 第一条红在「flux.session 被清掉」；
 * 游客与成功两条正控两边都绿。
 */

class FakeEventSource {
  static last: FakeEventSource | null = null;
  url: string;
  onerror: (() => void) | null = null;
  constructor(url: string) {
    this.url = url;
    FakeEventSource.last = this;
  }
  addEventListener() {}
  close() {}
}

function stubProfileFetch() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({
      status: 200,
      headers: new Map([["content-type", "application/json"]]),
      json: async () => ({ data: { modules: {}, topbar_spark_chip: true } }),
    }) as unknown as Response),
  );
}

async function mount() {
  const { UserMenu } = await import("@/components/user-menu");
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <UserMenu loginLabel="登录" />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  cleanup();
  document.cookie = "flux.session=1; path=/";
  localStorage.clear();
  FakeEventSource.last = null;
  vi.stubGlobal("EventSource", FakeEventSource);
  stubProfileFetch();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("顶栏 /me 取数失败（P-611）", () => {
  it("不清会话标记与本地用户，也不画登出入口", async () => {
    const { api } = await import("@/lib/api-client");
    vi.spyOn(api, "get").mockRejectedValue(new Error("boom"));
    localStorage.setItem("flux.user", '{"username":"root"}');
    await mount();
    await waitFor(() =>
      expect(api.get).toHaveBeenCalledWith("/api/v1/me"),
    );
    expect(document.cookie).toContain("flux.session=1");
    expect(localStorage.getItem("flux.user")).toBe('{"username":"root"}');
    await waitFor(() =>
      expect(document.querySelector(".usermenu-login")).toBeNull(),
    );
  });

  it("正控：游客没有会话标记时仍显示登录入口，也不打 /me", async () => {
    document.cookie = "flux.session=; max-age=0; path=/";
    const { api } = await import("@/lib/api-client");
    const spy = vi.spyOn(api, "get");
    await mount();
    expect(document.querySelector(".usermenu-login")?.textContent).toBe(
      "登录",
    );
    expect(spy).not.toHaveBeenCalled();
  });

  it("正控：取数成功照常渲染用户名与未读角标", async () => {
    const { api } = await import("@/lib/api-client");
    vi.spyOn(api, "get").mockImplementation(async (path: string) => {
      if (path === "/api/v1/me") {
        return {
          id: 1,
          username: "root",
          uploaded: 0,
          downloaded: 0,
          seeding: 0,
          leeching: 0,
          unread_messages: 3,
        } as never;
      }
      return { balance: 5 } as never;
    });
    await mount();
    await waitFor(() =>
      expect(document.querySelector(".usermenu__dot")?.textContent).toBe(
        "3",
      ),
    );
    expect(document.cookie).toContain("flux.session=1");
  });
});
