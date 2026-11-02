import { describe, expect, it } from "vitest";
import { librarySub, LIB_CAT_CAP } from "@/lib/nav-menu";

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

/**
 * 分类上限与内置包体量（0455：综合站加到 13 类后，上限 12 会把
 * 「教育资源」从导航里截掉——内置分类被自己的上限截掉不是降级设计）。
 * 这条与 scripts/pt_audit_m_cat_key_audit.py 的 M5 成对：那边读库里的
 * 包分类数，这边钉常量本身。
 */
describe("LIB_CAT_CAP 与内置包", () => {
  it("上限必须容得下内置最大包（综合站 13 类）", () => {
    expect(LIB_CAT_CAP).toBeGreaterThanOrEqual(13);
  });

  it("恰好等于上限：全出现，且不多出一条『更多分类…』", () => {
    const atCap = Array.from({ length: LIB_CAT_CAP }, (_, i) => ({
      id: i + 1,
      name: `c${i + 1}`,
    }));
    const g = librarySub(nav as never, atCap as never);
    // 每组 = 1 个「全部」叶子 + cap 个分类叶子
    expect(g[0].items?.length).toBe(LIB_CAT_CAP + 1);
    expect(
      g.every((x) => !(x.items ?? []).some((i) => i.label === "更多分类…")),
    ).toBe(true);
  });

  /** 超上限的分类不能只是「消失」（0455 批站长报「综合站还缺三类」的真因）：
   *  merge 模式下站点把历史各包的分类都留着（现网 70 条），cap 之后那些
   *  后台在册、列表页能筛，唯独导航没入口。现在补一条落回列表页，
   *  三档过滤各带自己的查询串。 */
  it("超上限：每组末尾补一条『更多分类…』，且带着本组的过滤串", () => {
    const atCap = Array.from({ length: LIB_CAT_CAP }, (_, i) => ({
      id: i + 1,
      name: `c${i + 1}`,
    }));
    const over = [...atCap, { id: 999, name: "c999" }];
    const navX = { ...nav, moreCats: "更多分类…" };
    const g2 = librarySub(navX as never, over as never);
    expect(g2[0].items?.length).toBe(LIB_CAT_CAP + 2);
    const tail = g2.map((g) => (g.items ?? []).slice(-1)[0]);
    expect(tail.map((i) => i?.href)).toEqual([
      "/torrents",
      "/torrents?official=0",
      "/torrents?official=1",
    ]);
    // 文案只从字典来（不许在 lib 里硬编码中文）
    expect(tail.every((i) => i?.label === "更多分类…")).toBe(true);
    // 被截掉的那条确实不在叶子里——它的入口就是这条「更多分类…」
    expect(
      g2[0].items?.some((i) => i.href.includes("category_id=999")),
    ).toBe(false);
  });

  it("字典缺 moreCats 时不挤出半个字（回落空串，由三语补齐保证有值）", () => {
    const over = Array.from({ length: LIB_CAT_CAP + 1 }, (_, i) => ({
      id: i + 1,
      name: `c${i + 1}`,
    }));
    const g3 = librarySub(nav as never, over as never);
    const last = (g3[0].items ?? []).slice(-1)[0];
    expect(last?.href).toBe("/torrents");
    expect(last?.label).toBe("");
  });
});
