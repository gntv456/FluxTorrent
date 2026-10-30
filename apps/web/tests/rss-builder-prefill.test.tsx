import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("next/navigation", () => ({
  useSearchParams: () => new URLSearchParams("categories=7"),
}));

import { RssBuilder } from "@/components/rss-builder";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

/** 资源页「订阅当前结果」把分类带到 getrss；旧的 cats 参数也兼容。 */
describe("RSS 分类预填", () => {
  it("categories=7 会勾选分类并进入生成的订阅链接", async () => {
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
        if (url.includes("/site-profile")) {
          return Promise.resolve(
            envelope({
              categories: [
                { id: 7, name: "电影" },
                { id: 8, name: "剧集" },
              ],
            }),
          );
        }
        return Promise.resolve(envelope({}));
      }),
    );
    const { container } = render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <RssBuilder />
      </LocaleProvider>,
    );
    const cat = await screen.findByLabelText("电影");
    expect((cat as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("剧集") as HTMLInputElement).checked).toBe(
      false,
    );
    expect(
      container.querySelector<HTMLInputElement>(
        'input[readonly][value*="categories=7"]',
      ),
    ).not.toBeNull();
    vi.unstubAllGlobals();
  });
});
