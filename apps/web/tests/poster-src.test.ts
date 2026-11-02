import { describe, expect, it } from "vitest";
import { posterSrc } from "@/lib/poster-src";

describe("posterSrc 代理改写", () => {
  it("豆瓣图床 URL 改写为代理", () => {
    const u = "https://img3.doubanio.com/view/photo/m_ratio_poster/public/p1.jpg";
    expect(posterSrc(u)).toBe(
      `/api/v1/works/poster-proxy?url=${encodeURIComponent(u)}`,
    );
  });
  it("tmdb 图床改写；子域匹配", () => {
    const u = "https://image.tmdb.org/t/p/w500/x.jpg";
    expect(posterSrc(u)?.startsWith("/api/v1/works/poster-proxy?url=")).toBe(
      true,
    );
  });
  it("非白名单域原样返回（不当开放代理）", () => {
    const u = "https://evil.com/a.jpg";
    expect(posterSrc(u)).toBe(u);
  });
  it("相对路径与空值原样", () => {
    expect(posterSrc("/local.png")).toBe("/local.png");
    expect(posterSrc(null)).toBeNull();
  });
  it("仿冒域不匹配（doubanio.com.evil.com）", () => {
    const u = "https://img3.doubanio.com.evil.com/a.jpg";
    expect(posterSrc(u)).toBe(u);
  });
});
