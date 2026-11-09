/** 2026-11-03 站长报两症状的回归：
 *  ① 点资源库三级菜单自动弹高级筛选面板（导航行为被当筛选行为）；
 *  ② 选「资源-电影」点亮四个按钮（子集匹配四路命中）。
 *  advAutoOpen 抽成纯函数、MainMenu 的 isActive 口径在此各钉一组判据。 */
import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";

// useSearchParams 按「最近一次 setQuery」动态返回：vi.mock 工厂在模块
// 加载期执行，拿不到用例里的动态 query，经 holder 中转
const spHolder: { sp: URLSearchParams } = { sp: new URLSearchParams() };
vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn() }),
  usePathname: () => "/torrents",
  useSearchParams: () => spHolder.sp,
}));

import { advAutoOpen } from "@/app/(main)/torrents/_parts/torrents-utils";
import { buildTorrentChips } from "@/app/(main)/torrents/_parts/torrents-chips";
import { MainMenu } from "@/components/main-menu";
import { zhCN } from "@/i18n/zh-CN";
import {
  buildNavTree,
  claimedPaths,
  isActive as nodeIsActive,
  leavesOf,
  pathToActive,
} from "@/lib/nav-tree";

// 资源库下拉形状（lib/nav-menu.ts librarySub 产物，取三组各两叶）
const sub = [
  {
    label: "全部",
    href: "/torrents",
    items: [
      { href: "/torrents", label: "全部" },
      { href: "/torrents?category_id=1", label: "电影" },
    ],
  },
  {
    label: "资源",
    href: "/torrents?official=0",
    items: [
      { href: "/torrents?official=0", label: "全部" },
      { href: "/torrents?category_id=1&official=0", label: "电影" },
    ],
  },
  {
    label: "官种",
    href: "/torrents?official=1",
    items: [
      { href: "/torrents?official=1", label: "全部" },
      { href: "/torrents?category_id=1&official=1", label: "电影" },
    ],
  },
];

const menuItems = [
  { href: "/", label: "首页" },
  { href: "/torrents", label: "资源库", sub },
];
const menuGroups = [
  {
    group: "更多",
    items: [{ href: "/torrents?official=1", label: "官种" }],
  },
];
/* 2026-11-07 导航重构：下拉形态归一成 NavNode 树，点亮口径迁到
 * lib/nav-tree.ts 的纯函数。原 DOM 断言改为树级断言（口径不变），
 * 另留一条 DOM 冒烟确认一级条仍输出 data-active。 */
const tree = buildNavTree(menuItems, menuGroups, "更多 ▾");
const leaves = leavesOf(tree);

function litLabels(query: string): string[] {
  const sp = new URLSearchParams(query);
  const claimed = claimedPaths(leaves, sp);
  const test = (href: string) =>
    nodeIsActive(href, "/torrents", sp, claimed);
  const parents = tree
    .filter(
      (n) =>
        n.children.length &&
        (test(n.href) ||
          leavesOf(n.children).some((c) => test(c.href))),
    )
    .map((n) => `${n.label} ▾`);
  const hits = leaves
    .filter((n) => !n.children.length && test(n.href))
    .map((n) => n.label);
  return parents.concat(hits);
}

describe("症状②：菜单点亮口径（键值全等）", () => {
  it("选「资源-电影」只点亮该叶子，全部-电影/资源-全部不再陪亮", () => {
    expect(litLabels("category_id=1&official=0")).toEqual([
      "资源库 ▾",
      "电影",
    ]);
  });

  it("全部-电影（单 category_id）与官种-电影（category_id+official=1）互不误亮", () => {
    expect(litLabels("category_id=1")).toEqual(["资源库 ▾", "电影"]);
    expect(litLabels("category_id=1&official=1")).toEqual([
      "资源库 ▾",
      "电影",
    ]);
  });

  it("裸 /torrents（无参数）点亮「资源库」与全部组叶子", () => {
    expect(litLabels("")).toEqual(["资源库 ▾", "全部"]);
  });

  it("翻页/视图参数不破坏点亮（第 3 页仍在资源-电影频道）", () => {
    expect(litLabels("category_id=1&official=0&cursor=abc&view=cards"))
      .toEqual(["资源库 ▾", "电影"]);
  });

  it("首屏 path 展开到当前选中叶子所在层", () => {
    const sp = new URLSearchParams("category_id=1&official=0");
    const claimed = claimedPaths(leaves, sp);
    const path = pathToActive(tree, (n) =>
      nodeIsActive(n.href, "/torrents", sp, claimed));
    expect(path.length).toBeGreaterThan(0);
    expect(path[0]).toBe("p1");
  });

  it("一级条仍输出 data-active（DOM 冒烟）", () => {
    spHolder.sp = new URLSearchParams("category_id=1&official=0");
    const { container } = render(
      <MainMenu
        items={menuItems}
        groups={menuGroups}
        moreLabel="更多 ▾"
        ariaLabel="主导航"
      />,
    );
    const lit = Array.from(
      container.querySelectorAll<HTMLElement>('[data-active="true"]'),
    ).map((el) => (el.textContent ?? "").trim());
    // 2026-11-09 级联下拉改版：一级条是 Link 直进，▾ 移入独立
    // span，textContent 带尾空白——断言改前缀包含
    expect(lit.some((t) => t.startsWith("资源库"))).toBe(true);
  });
});

describe("症状①：纯导航落地不自动展开高级面板", () => {
  const chipsOf = (sp: Record<string, string | undefined>) =>
    buildTorrentChips({
      sp,
      dict: zhCN,
      categories: [{ id: 1, label: "电影" }],
      tags: [],
      secDict: null,
      dimKinds: [],
      withParam: () => "/torrents",
    });

  it("资源-电影（category_id+official）不展开", () => {
    const sp = { category_id: "1", official: "0" };
    const chips = chipsOf(sp).filter((c) => c.key !== "search");
    expect(chips.length).toBeGreaterThan(0); // chip 仍展示（真实筛选态）
    expect(advAutoOpen(sp, chips.length)).toBe(false);
  });

  it("带真筛选参数（owner/promo）照常展开", () => {
    const sp = { category_id: "1", owner: "alice" };
    expect(advAutoOpen(sp, 2)).toBe(true);
  });

  it("无 chip 不展开（原语义保持）", () => {
    expect(advAutoOpen({}, 0)).toBe(false);
  });
});
