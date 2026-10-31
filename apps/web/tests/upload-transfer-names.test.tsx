import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { UploadForm } from "@/components/upload-form";

/**
 * 转种契约（auto_feed 适配版）：发布表单的字段名是外部油猴脚本的定位锚点。
 * 这些 name 属性不是给原生提交用的（表单自己拼 FormData），而是给
 * scripts/userscripts/auto-feed-flux 用的稳定接口——重构时删掉/改名会
 * 让「从别站转载到本站」静默失去自动填充，所以在这里钉死。
 */

function ok(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({
      code: 0,
      message: "ok",
      data,
      request_id: "test",
    }),
  };
}

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("/api/v1/site-profile")) {
        return ok({
          categories: [{ id: 1, name: "电影" }],
          metadata_sources: ["imdb", "douban", "tmdb", "mediainfo"],
        });
      }
      if (url.includes("/api/v1/tags-dict")) {
        return ok({ tags: [], groups: [] });
      }
      if (url.includes("/api/v1/section-dict")) return ok({ kinds: [] });
      if (url.includes("/api/v1/me/csrf")) return ok({});
      return ok(null);
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("上传表单的转种字段契约", () => {
  it("暴露 auto_feed 适配层依赖的全部 name 属性", async () => {
    const { container } = render(<UploadForm />);
    await waitFor(() => {
      expect(container.querySelector('[name="category_id"]')).not.toBeNull();
    });
    const names = [
      "file",
      "name",
      "small_descr",
      "imdb",
      "douban",
      "tmdb",
      "ptgen_url",
      "nfo",
      "attach",
      "mediainfo",
      "price",
      "poster",
      "descr",
      "category_id",
    ];
    for (const name of names) {
      expect(
        container.querySelector(`[name="${name}"]`),
        `缺少 name="${name}"（会断掉转种自动填充）`,
      ).not.toBeNull();
    }
  });
});
