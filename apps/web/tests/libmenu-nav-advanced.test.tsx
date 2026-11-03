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

/** 渲染 MainMenu 并收集 data-active 的条目文案（query 经 spHolder 注入） */
function renderMenu(query: string): string[] {
  spHolder.sp = new URLSearchParams(query);
  const { container } = render(
    <MainMenu
      items={[
        { href: "/", label: "首页" },
        { href: "/torrents", label: "资源库", sub },
      ]}
      groups={[
        {
          group: "更多",
          items: [{ href: "/torrents?official=1", label: "官种" }],
        },
      ]}
      moreLabel="更多 ▾"
      ariaLabel="主导航"
    />,
  );
  return Array.from(
    container.querySelectorAll<HTMLElement>('[data-active="true"]'),
  ).map((el) => (el.textContent ?? "").trim());
}

describe("症状②：菜单点亮口径（键值全等）", () => {
  it("选「资源-电影」只点亮该叶子，全部-电影/资源-全部不再陪亮", () => {
    const lit = renderMenu("category_id=1&official=0");
    expect(lit).toEqual(["资源库 ▾", "电影"]);
  });

  it("全部-电影（单 category_id）与官种-电影（category_id+official=1）互不误亮", () => {
    expect(renderMenu("category_id=1")).toEqual(["资源库 ▾", "电影"]);
    expect(renderMenu("category_id=1&official=1")).toEqual([
      "资源库 ▾",
      "电影",
    ]);
  });

  it("裸 /torrents（无参数）点亮「资源库」与全部组叶子", () => {
    const lit = renderMenu("");
    expect(lit).toEqual(["资源库 ▾", "全部"]);
  });

  it("翻页/视图参数不破坏点亮（第 3 页仍在资源-电影频道）", () => {
    const lit = renderMenu("category_id=1&official=0&cursor=abc&view=cards");
    expect(lit).toEqual(["资源库 ▾", "电影"]);
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
