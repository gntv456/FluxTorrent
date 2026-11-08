/** 导航画布交互回归（2026-11-07）：点击逐级展开、面包屑回退、
 *  跨层级搜索、卷轴题签。两套渲染壳共用同一棵 NavNode 树与 path。 */
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import type { ReactNode } from "react";
import { NavColumns, NavScroll } from "@/components/nav-canvas";
import type { NavNode } from "@/lib/nav-tree";

vi.mock("next/link", () => ({
  default: (p: {
    href: string;
    children: ReactNode;
    className?: string;
  }) => (
    <a href={p.href} className={p.className}>{p.children}</a>
  ),
}));

const tree: NavNode[] = [
  { id: "p0", label: "首页", href: "/", children: [] },
  {
    id: "p1",
    label: "教育资源",
    href: "/torrents?category_id=13",
    children: [
      {
        id: "s0",
        label: "小学",
        href: "/torrents?category_id=102",
        children: [
          {
            id: "g0",
            label: "三年级",
            href: "/torrents?grade_id=3",
            children: [
              {
                id: "m0",
                label: "数学",
                href: "/torrents?sec_subject=100",
                children: [],
              },
            ],
          },
        ],
      },
    ],
  },
];

function setup(Scroll: boolean) {
  const onNavigate = vi.fn();
  const onClose = vi.fn();
  const props = {
    tree,
    open: true,
    rootId: "p1",
    active: () => false,
    onNavigate,
    onClose,
    goLabel: "进入",
  };
  const view = render(Scroll
    ? <NavScroll {...props} />
    : <NavColumns {...props} />);
  return { ...view, onNavigate, onClose };
}

const byText = (root: HTMLElement, sel: string, text: string) =>
  Array.from(root.querySelectorAll<HTMLElement>(sel)).find(
    (el) => (el.textContent ?? "").trim().includes(text),
  );

describe("NavColumns · 点击逐级追加列", () => {
  it("rootId 进入 1 列（二级），点二级出三级列，点三级出四级列", () => {
    const { container } = setup(false);
    expect(container.querySelectorAll(".nvc__col").length).toBe(1);
    const grade = byText(container, ".nvc__row", "小学");
    expect(grade).toBeTruthy();
    fireEvent.click(grade!);
    expect(container.querySelectorAll(".nvc__col").length).toBe(2);
    fireEvent.click(byText(container, ".nvc__row", "三年级")!);
    expect(container.querySelectorAll(".nvc__col").length).toBe(3);
    expect(byText(container, ".nvc__row", "数学")).toBeTruthy();
  });

  it("2026-11-08 症状②：rootId 进入第一列直达二级，不重复一级平铺", () => {
    const { container } = setup(false);
    // 第一列不该再有「首页」等一级项，直接是「小学」等二级
    const firstCol = container.querySelector<HTMLElement>(".nvc__col")!;
    expect(byText(firstCol, ".nvc__row", "小学")).toBeTruthy();
    expect(byText(firstCol, ".nvc__row", "首页")).toBeFalsy();
    expect(firstCol.querySelector(".nvc__cap span")?.textContent)
      .toBe("L2");
  });

  it("2026-11-08 症状①：状态条「进入」一级层也能直进", () => {
    const { container, onNavigate } = setup(false);
    // 初始 path=[p1]：状态条显示「教育资源」并带进入按钮
    const status = container.querySelector(".nvc__status")!;
    expect(status.textContent).toContain("教育资源");
    fireEvent.click(status.querySelector(".nvc__status-go")!);
    expect(onNavigate).toHaveBeenCalledWith("/torrents?category_id=13");
  });

  it("状态条跟随展开层（钻到三年级后进入年级页）", () => {
    const { container, onNavigate } = setup(false);
    fireEvent.click(byText(container, ".nvc__row", "小学")!);
    fireEvent.click(byText(container, ".nvc__row", "三年级")!);
    const status = container.querySelector(".nvc__status")!;
    expect(status.textContent).toContain("三年级");
    fireEvent.click(status.querySelector(".nvc__status-go")!);
    expect(onNavigate).toHaveBeenCalledWith("/torrents?grade_id=3");
  });

  it("点击面包屑回退到该级（全站=收回到一级全列）", () => {
    const { container } = setup(false);
    fireEvent.click(byText(container, ".nvc__row", "小学")!);
    expect(container.querySelectorAll(".nvc__col").length).toBe(2);
    fireEvent.click(byText(container, ".nvc__crumb button", "全站")!);
    // 「全站」回退到一级列：此时列 = 全部一级项平铺
    const cols = container.querySelectorAll<HTMLElement>(".nvc__col");
    expect(cols.length).toBe(1);
    expect(byText(cols[0], ".nvc__row", "首页")).toBeTruthy();
  });

  it("点叶子触发 onNavigate", () => {
    const { container, onNavigate } = setup(false);
    fireEvent.click(byText(container, ".nvc__row", "小学")!);
    fireEvent.click(byText(container, ".nvc__row", "三年级")!);
    fireEvent.click(byText(container, ".nvc__row", "数学")!);
    expect(onNavigate).toHaveBeenCalledWith("/torrents?sec_subject=100");
  });

  it("搜索跨层级命中并展示完整路径", () => {
    const { container } = setup(false);
    const input = container.querySelector<HTMLInputElement>(".nvc__search");
    fireEvent.change(input!, { target: { value: "数学" } });
    expect(container.querySelectorAll(".nvc__res").length).toBeGreaterThan(0);
    expect(container.querySelector(".nvc__res-path")?.textContent)
      .toContain("教育资源");
  });
});

describe("NavScroll · 卷轴逐级展卷", () => {
  it("点册页追加一卷题签，当前卷列出子级", () => {
    const { container } = setup(true);
    expect(container.querySelectorAll(".nvc__strip").length).toBe(1);
    fireEvent.click(byText(container, ".nvc__tile", "小学")!);
    expect(container.querySelectorAll(".nvc__strip").length).toBe(2);
    expect(byText(container, ".nvc__tile", "三年级")).toBeTruthy();
  });

  it("点题签回到该卷", () => {
    const { container } = setup(true);
    fireEvent.click(byText(container, ".nvc__tile", "小学")!);
    fireEvent.click(byText(container, ".nvc__strip", "教育资源")!);
    expect(byText(container, ".nvc__tile", "小学")).toBeTruthy();
  });

  it("rootId 进入首卷直达二级；状态条一级层可直进", () => {
    const { container, onNavigate } = setup(true);
    // 首卷册页直接是二级「小学」，无一级「首页」混入
    expect(byText(container, ".nvc__tile", "小学")).toBeTruthy();
    expect(byText(container, ".nvc__tile", "首页")).toBeFalsy();
    const status = container.querySelector(".nvc__status")!;
    fireEvent.click(status.querySelector(".nvc__status-go")!);
    expect(onNavigate).toHaveBeenCalledWith("/torrents?category_id=13");
  });
});
