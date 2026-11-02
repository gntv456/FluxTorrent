import { describe, expect, it } from "vitest";
import { backgroundOpacity, cssBackgroundImage } from "@/lib/site-background";

/** 背景图设置 → CSS 值：收 http(s) 绝对地址与站内相对路径（0449，
 *  相对路径随访问入口同源加载），引号/空白/伪协议/协议相对一律拒绝
 *  （值会拼进 url("…")；后台 rule=url_or_path 是第一道，这里是最后一道）。 */
describe("背景图 CSS 归一", () => {
  it("http(s) 绝对地址 → url(\"…\")", () => {
    expect(cssBackgroundImage(" https://cdn.example/bg.jpg ")).toBe(
      'url("https://cdn.example/bg.jpg")',
    );
    expect(cssBackgroundImage("http://x/bg.png?a=1")).toBe(
      'url("http://x/bg.png?a=1")',
    );
  });

  it("站内相对路径 → 原样保留（同源加载）", () => {
    expect(cssBackgroundImage("/backgrounds/site-tide-photo.jpg")).toBe(
      'url("/backgrounds/site-tide-photo.jpg")',
    );
  });

  it("协议相对/伪协议/引号注入/空白/超长 → 不渲染（负控）", () => {
    const bad = [
      "",
      null,
      undefined,
      "//evil.example/bg.jpg",
      "/a\".jpg",
      "/a b.jpg",
      "cdn.example/bg.jpg",
      "javascript:alert(1)",
      'https://x/a".jpg',
      "https://x/a b.jpg",
      "data:image/png;base64,AAAA",
      "https://x/" + "a".repeat(2100),
    ];
    for (const v of bad) {
      expect(cssBackgroundImage(v)).toBeNull();
    }
  });

  it("不透明度 0~100 夹紧，非法值回落默认", () => {
    expect(backgroundOpacity(30, 25)).toBe(30);
    expect(backgroundOpacity(-5, 25)).toBe(0);
    expect(backgroundOpacity(140, 25)).toBe(100);
    expect(backgroundOpacity(undefined, 25)).toBe(25);
    expect(backgroundOpacity(Number.NaN, 25)).toBe(25);
  });
});
