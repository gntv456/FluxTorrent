import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { PushSettings } from "@/components/push-settings";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 推送设置面板：vapid-key / notice-prefs 取数失败不能演成
 *  「服务端未配置 VAPID 密钥」，也不能让三个推送开关静默消失。 */

afterEach(() => vi.unstubAllGlobals());

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

function stubByPath(
  vapid: () => Promise<Response>,
  prefs: () => Promise<Response>,
) {
  vi.stubGlobal(
    "fetch",
    vi.fn((url: string) => {
      if (url.includes("/push/vapid-key")) return vapid();
      if (url.includes("/me/notice-prefs")) return prefs();
      return Promise.resolve(envelope(null));
    }),
  );
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <PushSettings />
    </LocaleProvider>,
  );
}

describe("推送设置失败不演错误陈述", () => {
  it("vapid-key 失败：报加载失败，不断言「服务端未配置」", async () => {
    stubByPath(
      () => Promise.reject(new Error("network down")),
      () => Promise.resolve(envelope({ push_hr: true })),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.push2.serverDisabled)).toBeNull();
  });

  it("notice-prefs 失败：三个开关有失败提示，订阅按钮仍可用", async () => {
    stubByPath(
      () => Promise.resolve(envelope({ public_key: "k", enabled: true })),
      () => Promise.reject(new Error("network down")),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryAllByRole("switch")).toHaveLength(0);
    expect(screen.getByRole("button", { name: zhCN.push2.btnSub })).toBeTruthy();
  });

  it("服务端确实关闭推送：仍显示「未配置 VAPID」", async () => {
    stubByPath(
      () => Promise.resolve(envelope({ public_key: "", enabled: false })),
      () => Promise.resolve(envelope({ push_hr: true })),
    );
    mount();
    expect(await screen.findByText(zhCN.push2.serverDisabled)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
