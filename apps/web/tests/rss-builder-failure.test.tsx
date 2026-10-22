import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { RssBuilder } from "@/components/rss-builder";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

vi.mock("next/navigation", () => ({
  useSearchParams: () => new URLSearchParams(),
}));

/** RSS 构建器：rss-info 取数失败不能演成「登录后查看…」——
 *  已登录会员会以为自己的会话没了。 */

afterEach(() => vi.unstubAllGlobals());

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <RssBuilder />
    </LocaleProvider>,
  );
}

describe("RSS 构建器失败不演登录提示", () => {
  it("rss-info 失败：报加载失败，不报「登录后查看」", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("network down");
      }),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(/登录后查看/)).toBeNull();
  });

  it("rss-info 成功：正常渲染生成结果", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) => {
        if (url.includes("/rss-info")) {
          return Promise.resolve(
            envelope({
              urls: [],
              passkey: "pk",
              base: "https://t.example/api/v1/rss/",
              tokens: [],
            }),
          );
        }
        return Promise.resolve(envelope({}));
      }),
    );
    mount();
    expect(await screen.findByText(zhCN.getrss.resultTitle)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
