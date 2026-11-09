/** 主菜单/子菜单入口完整性（2026-11-09 验收 1）：
 *  导航数据同源构建后，每一个叶子入口必须同时能在「桌面顶栏级联」
 *  与「<lg 抽屉树」两条路径里构造出来；默认 6 组 38 条入口全覆盖。
 *  以后增删入口时此用例会自然报警——若是有意变更，同步更新 38 这个
 *  基线数字（而不是删断言）。 */
import { describe, expect, it, vi, afterEach } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import { zhCN } from "@/i18n/zh-CN";

vi.mock("next/navigation", () => ({
  usePathname: () => "/",
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
    prefetch: vi.fn(),
    back: vi.fn(),
  }),
  useSearchParams: () => new URLSearchParams(),
}));

import { MainMenu } from "@/components/main-menu";
import { NavDrawer } from "@/components/nav-drawer";
import { defaultNav } from "@/lib/nav-menu";
import { buildNavTree, leavesOf, type NavNode } from "@/lib/nav-tree";

const built = defaultNav({
  nav: zhCN.nav,
  tabbar: zhCN.tabbar,
  currency: "魔力",
  modules: () => true,
  customItems: [],
  // 含教育分类 13：教育资源一级项挂出，抽屉必须能下钻到年级/科目
  libraryCats: [
    { id: 13, name: "教育资源" },
    { id: 1, name: "电影" },
    { id: 2, name: "剧集" },
  ],
});

const moreLabel = `${zhCN.nav.more} ▾`;
const tree = buildNavTree(built.primary, built.groups, moreLabel);
const leaves = leavesOf(tree).filter((n) => n.children.length === 0);
const branches = leavesOf(tree).filter((n) => n.children.length > 0);

function anchorHrefs(root: ParentNode): Set<string> {
  return new Set(
    Array.from(root.querySelectorAll("a"))
      .map((a) => a.getAttribute("href"))
      .filter((h): h is string => !!h),
  );
}

afterEach(cleanup);

describe("导航入口完整性（桌面 + <lg 抽屉）", () => {
  it("默认配置仍是 6 组 38 条入口（有意增删请同步本基线）", () => {
    expect(built.groups.map((g) => g.group)).toHaveLength(6);
    expect(
      built.groups.reduce((n, g) => n + g.items.length, 0),
    ).toBe(38);
  });

  it("桌面顶栏级联：每个叶子 href 都在 DOM 中，且父级展开语义齐全", () => {
    const { container } = render(
      <MainMenu
        items={built.primary}
        groups={built.groups}
        moreLabel={moreLabel}
        ariaLabel={zhCN.nav.ariaPrimary}
      />,
    );
    const hrefs = anchorHrefs(container);
    for (const leaf of leaves) {
      expect(hrefs.has(leaf.href), `桌面缺入口：${leaf.label} ${leaf.href}`).toBe(
        true,
      );
    }
    // 带子级的一级条必须有 aria-haspopup/aria-expanded（I1）
    const triggers = container.querySelectorAll(
      ".mainmenu-link[aria-haspopup]",
    );
    expect(triggers.length).toBeGreaterThan(0);
    for (const t of Array.from(triggers)) {
      expect(t.getAttribute("aria-expanded")).toMatch(/true|false/);
    }
  });

  it("<lg 抽屉树：每个叶子可达，每个分组的展开按钮与面板一一对应", async () => {
    render(
      <NavDrawer
        open
        onClose={() => {}}
        primary={built.primary}
        groups={built.groups}
        summary={null}
      />,
    );
    await waitFor(() =>
      expect(document.body.querySelector(".navdrawer")).toBeTruthy(),
    );
    const hrefs = anchorHrefs(document.body);
    for (const leaf of leaves) {
      expect(hrefs.has(leaf.href), `抽屉缺入口：${leaf.label} ${leaf.href}`).toBe(
        true,
      );
    }
    // 祖先链可构造：每个分支都同时有 aria-controls 按钮与对应的 ul 面板
    for (const node of branches as NavNode[]) {
      const panel = document.getElementById(`navd-${node.id}`);
      const toggle = document.querySelector(
        `[aria-controls="navd-${node.id}"]`,
      );
      expect(panel, `抽屉缺面板：${node.label}`).toBeTruthy();
      expect(toggle, `抽屉缺展开按钮：${node.label}`).toBeTruthy();
      expect(toggle?.getAttribute("aria-expanded")).toMatch(/true|false/);
    }
  });
});
