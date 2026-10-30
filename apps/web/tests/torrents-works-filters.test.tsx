import { describe, expect, it, vi } from "vitest";

const calls: string[] = [];
vi.mock("@/lib/api-client", () => ({
  api: {
    get: vi.fn(async (path: string) => {
      calls.push(path);
      return { items: [], total: 0, limit: 20, page: 1 };
    }),
  },
  ApiError: class ApiError extends Error {},
}));

import { TorrentWorks } from "@/app/(main)/torrents/_parts/torrents-works";
import { zhCN } from "@/i18n/zh-CN";

/** 作品聚合必须把列表页的筛选原样带到 /works；approval 除外
 *  （作品层只展示过审资源，后端会拒绝审核状态视图）。 */
describe("作品聚合筛选转发", () => {
  it("q/分类/标签/数值/维度/视角筛选都进入 works 请求", async () => {
    calls.length = 0;
    const sp = {
      p: "2",
      search: "阿凡达",
      search_area: "1",
      search_mode: "2",
      category_id: "1,2",
      official: "1",
      alive: "2",
      tag_ids: "3",
      tag_mode: "all",
      size_min: "1GB",
      date_from: "2026-10-01",
      min_seeders: "2",
      max_completed: "500",
      exclude: "cam",
      promo: "free,x2",
      owner: "flux",
      mine: "1",
      bookmarked: "1",
      anonymous: "1",
      rating_min: "8",
      sort: "seeders_asc",
      sec_media: "3",
      approval: "2",
    };
    await TorrentWorks({
      dict: zhCN,
      locale: "zh-CN",
      sp,
      cats: [],
      view: "card",
      perPage: 20,
      withParam: () => "/torrents",
    });
    expect(calls).toHaveLength(1);
    const url = new URL(calls[0], "http://x");
    expect(url.pathname).toBe("/api/v1/works");
    const p = url.searchParams;
    expect(p.get("q")).toBe("阿凡达");
    expect(p.get("search_area")).toBe("1");
    expect(p.get("search_mode")).toBe("2");
    expect(p.get("category_id")).toBe("1,2");
    expect(p.get("official")).toBe("1");
    expect(p.get("alive")).toBe("2");
    expect(p.get("tag_ids")).toBe("3");
    expect(p.get("tag_mode")).toBe("all");
    expect(p.get("size_min")).toBe("1GB");
    expect(p.get("date_from")).toBe("2026-10-01");
    expect(p.get("min_seeders")).toBe("2");
    expect(p.get("max_completed")).toBe("500");
    expect(p.get("exclude")).toBe("cam");
    expect(p.get("promo")).toBe("free,x2");
    expect(p.get("owner")).toBe("flux");
    expect(p.get("mine")).toBe("1");
    expect(p.get("bookmarked")).toBe("1");
    expect(p.get("anonymous")).toBe("1");
    expect(p.get("rating_min")).toBe("8");
    expect(p.get("sort")).toBe("seeders_asc");
    expect(p.get("sec_media")).toBe("3");
    expect(p.get("limit")).toBe("20");
    expect(p.get("page")).toBe("2");
    expect(p.get("approval")).toBeNull();
  });
});
