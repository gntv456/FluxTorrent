import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { zhCN } from "@/i18n/zh-CN";
import { LocaleProvider } from "@/i18n/client";

/** P-593：SSE 未读帧必须真的推动顶栏角标。
 *  旧实现 effect 依赖 []，闭包里的 me 永远是首渲染的 null，
 *  `if (n !== undefined && me)` 把每一帧都丢掉 ⇒ 页面开着时角标不动。 */

class FakeEventSource {
  static last: FakeEventSource | null = null;
  url: string;
  onerror: (() => void) | null = null;
  closed = false;
  private listeners = new Map<string, ((ev: MessageEvent) => void)[]>();
  constructor(url: string) {
    this.url = url;
    FakeEventSource.last = this;
  }
  addEventListener(type: string, cb: (ev: MessageEvent) => void) {
    const list = this.listeners.get(type) ?? [];
    list.push(cb);
    this.listeners.set(type, list);
  }
  close() {
    this.closed = true;
  }
  emit(type: string, data: unknown) {
    for (const cb of this.listeners.get(type) ?? []) {
      cb({ data: JSON.stringify(data) } as MessageEvent);
    }
  }
}

function stubProfileFetch() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      ({
        status: 200,
        headers: new Map([["content-type", "application/json"]]),
        json: async () => ({ data: { modules: {}, topbar_spark_chip: true } }),
      }) as unknown as Response,
    ),
  );
}

beforeEach(() => {
  cleanup();
  document.cookie = "flux.session=1; path=/";
  FakeEventSource.last = null;
  vi.stubGlobal("EventSource", FakeEventSource);
  stubProfileFetch();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("顶栏未读 SSE", () => {
  it("收到 unread 帧后角标从 2 变 7", async () => {
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
          unread_messages: 2,
        } as never;
      }
      return { balance: 5 } as never;
    });
    const { UserMenu } = await import("@/components/user-menu");
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <UserMenu loginLabel="登录" />
      </LocaleProvider>,
    );
    await waitFor(() =>
      expect(document.querySelector(".usermenu__dot")?.textContent).toBe("2"),
    );
    const es = FakeEventSource.last;
    expect(es?.url).toBe("/api/v1/me/messages/stream");
    act(() => es!.emit("unread", { count: 7 }));
    await waitFor(() =>
      expect(document.querySelector(".usermenu__dot")?.textContent).toBe("7"),
    );
  });
});
