import { describe, expect, it } from "vitest";
import { librarySub } from "@/lib/nav-menu";

/** 资源库三级下拉（2895860c）的链接形状回归。
 *  站长报「资源/官种 下的『全部』点了 404」：那两条渲染成
 *  `/torrents&official=0`（缺 `?`，整段被当路径）。 */
const nav = { libAll: "全部", libRes: "资源", official: "官种" };
const cats = [
  { id: 7, name: "电影" },
  { id: 9, name: "动画" },
];
const groups = librarySub(nav as never, cats as never);
const flat = groups.flatMap((g) => [
  g.href,
  ...(g.items ?? []).map((i) => i.href),
]);

describe("librarySub 链接", () => {
  it("每条链接要么是 /torrents，要么带 ? 查询串（不许出现裸 &）", () => {
    for (const href of flat) {
      expect(href, href).toMatch(/^\/torrents(\?[^&\s]*(&[^?\s]+)*)?$/);
      expect(href.includes("/torrents&"), href).toBe(false);
    }
  });

  it("二级三组与三级『全部』的 official 三态一一对应", () => {
    expect(groups.map((g) => g.href)).toEqual([
      "/torrents",
      "/torrents?official=0",
      "/torrents?official=1",
    ]);
    expect(groups[1].items?.[0].href).toBe("/torrents?official=0");
    expect(groups[2].items?.[0].href).toBe("/torrents?official=1");
    // 「全部」组不过滤
    expect(groups[0].items?.[0].href).toBe("/torrents");
  });

  it("分类叶子在带过滤时把参数并到 category_id 之后", () => {
    expect(groups[2].items?.[1].href).toBe(
      "/torrents?category_id=7&official=1",
    );
    expect(groups[0].items?.[1].href).toBe("/torrents?category_id=7");
  });

  it("负控：把修复退回旧拼法，断言必须红", () => {
    const buggy = `/torrents&official=1`;
    expect(buggy.includes("/torrents&")).toBe(true);
    expect(buggy).not.toMatch(/^\/torrents\?/);
  });
});
