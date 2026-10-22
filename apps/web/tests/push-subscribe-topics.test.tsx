import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { api } from "@/lib/api-client";
import { PushSettings } from "@/components/push-settings";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * Web Push 订阅必须登记 hr 话题（P-370 的客户端另一半）。
 *
 * 后端投递侧 push_notify.rs 用 topic="hr" 发 H&R 预警/违规，
 * `push_to_user` 按 `$topic = ANY(p.topics)` 过滤；订阅端显式传
 * ["promo","request","message"] 会覆盖后端 default_topics，于是
 * H&R 推送结构上永远送不到。负控：去掉数组里的 "hr" 本用例必红。
 */
const SUB = {
  toJSON: () => ({
    endpoint: "https://push.example.test/ep-1",
    keys: { p256dh: "AAAA", auth: "BBBB" },
  }),
};

let post: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  vi.spyOn(api, "get").mockResolvedValue({
    public_key: "AAAA",
    enabled: true,
  } as never);
  post = vi.spyOn(api, "post").mockResolvedValue({} as never);
  Object.defineProperty(navigator, "serviceWorker", {
    configurable: true,
    value: {
      ready: Promise.resolve({
        pushManager: { subscribe: async () => SUB },
      }),
      getRegistration: async () => ({
        pushManager: { getSubscription: async () => null },
      }),
    },
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  delete (navigator as { serviceWorker?: unknown }).serviceWorker;
});

describe("推送订阅话题", () => {
  it("订阅载荷包含 H&R（hr），不能只订三类", async () => {
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <PushSettings />
      </LocaleProvider>,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: zhCN.push2.btnSub }),
    );
    await waitFor(() => expect(post).toHaveBeenCalled());
    const call = post.mock.calls.find(
      (c: unknown[]) => c[0] === "/api/v1/push/subscribe",
    );
    expect(call).toBeTruthy();
    expect((call?.[1] as { topics: string[] }).topics).toEqual([
      "promo",
      "request",
      "message",
      "hr",
    ]);
  });

  it("逐类推送开关写回 /me/notice-prefs（P-574）", async () => {
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <PushSettings />
      </LocaleProvider>,
    );
    const sw = await screen.findByRole("switch", {
      name: zhCN.push2.prefs.push_hr,
    });
    await userEvent.click(sw);
    await waitFor(() => expect(post).toHaveBeenCalled());
    const call = post.mock.calls.find(
      (c: unknown[]) => c[0] === "/api/v1/me/notice-prefs",
    );
    expect(call).toBeTruthy();
    expect(call?.[1]).toEqual({ key: "push_hr", enabled: false });
  });
});
