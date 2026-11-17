/** wiki-index/wiki-search 渐进增强检索（0495 / dev.16 RSC 直出批）回归：
 *  卡片由 RSC 直出、检索是 client 件按 data-* 做 DOM 过滤——无 JS 时
 *  目录全量可见。这里钉住两件事：
 *  1) WikiIndex（RSC）直出所有分组卡与 data-wiki-card/data-title 锚点；
 *  2) WikiSearch 击键后命不中的卡 display:none、整组空时分组卡隐藏、
 *     全空时空态提示出现。 */

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { WikiIndex } from "@/components/wiki-index";
import { WikiSearch } from "@/components/wiki-search";

const entries = [
  {
    id: 1,
    slug: "start",
    title: "新手入门",
    section: "start",
    updated_at: "2026-11-17T00:00:00Z",
  },
  {
    id: 2,
    slug: "rules",
    title: "站规",
    section: "start",
    updated_at: "2026-11-16T00:00:00Z",
  },
  {
    id: 3,
    slug: "api-integration",
    title: "API 接入",
    section: "api",
    updated_at: "2026-11-17T00:00:00Z",
  },
];
const sections = [
  { key: "start", title: "入门", desc: "起步" },
  { key: "api", title: "接口", desc: "开发" },
];

function renderIndex() {
  return render(
    <WikiIndex
      entries={entries}
      sections={sections}
      searchPlaceholder="搜索 Wiki"
      searchEmpty="没有匹配的页面"
      updatedAtLabel="更新"
      pagesUnit={(n) => `${n} 页`}
    />,
  );
}

afterEach(cleanup);

describe("WikiIndex（RSC 直出）", () => {
  it("服务端直出全部卡片（无 JS 可见），带 data-title/data-slug 锚点", () => {
    renderIndex();
    for (const e of entries) {
      const link = screen.getByText(e.title).closest("a");
      expect(link).not.toBeNull();
      expect(link?.getAttribute("data-wiki-card")).toBe(e.slug);
      expect(link?.getAttribute("data-title")).toBe(e.title);
    }
    // 分组卡锚点：过滤命中空时整卡隐藏的依据
    expect(document.querySelectorAll("[data-wiki-section]").length).toBe(2);
  });

  it("检索框与空态占位（display:none）直出", () => {
    renderIndex();
    expect(
      screen.getByPlaceholderText("搜索 Wiki").getAttribute("type"),
    ).toBe("search");
    const empty = document.getElementById("wiki-filter-empty");
    expect(empty).not.toBeNull();
    expect(empty?.style.display).toBe("none");
  });
});

describe("WikiSearch（DOM 渐进增强过滤）", () => {
  it("关键词命中：命不中的卡隐藏、命中空的分组卡隐藏、全空显示空态", () => {
    renderIndex();
    const input = screen.getByPlaceholderText(
      "搜索 Wiki",
    ) as HTMLInputElement;
    // 命中 1 条（API 接入）→ start 组两卡全隐、start 分组卡隐藏、api 分组卡在
    fireEvent.change(input, { target: { value: "api" } });
    const cardOf = (slug: string) =>
      document.querySelector<HTMLElement>(`[data-wiki-card="${slug}"]`);
    expect(cardOf("start")?.style.display).toBe("none");
    expect(cardOf("rules")?.style.display).toBe("none");
    expect(cardOf("api-integration")?.style.display).toBe("");
    const secOf = (key: string) =>
      document.querySelector<HTMLElement>(`[data-wiki-section="${key}"]`);
    expect(secOf("start")?.style.display).toBe("none");
    expect(secOf("api")?.style.display).toBe("");
    expect(document.getElementById("wiki-filter-empty")?.style.display).toBe(
      "none",
    );

    // 无人命中 → 空态出现
    fireEvent.change(input, { target: { value: "zzz不存在" } });
    expect(document.getElementById("wiki-filter-empty")?.style.display).toBe(
      "",
    );
    expect(secOf("api")?.style.display).toBe("none");

    // 清空 → 全量恢复
    fireEvent.change(input, { target: { value: "" } });
    expect(cardOf("start")?.style.display).toBe("");
    expect(secOf("start")?.style.display).toBe("");
    expect(document.getElementById("wiki-filter-empty")?.style.display).toBe(
      "none",
    );
  });

  it("标题不匹配但 slug 匹配仍命中（data-slug 兜底链）", () => {
    renderIndex();
    const input = screen.getByPlaceholderText(
      "搜索 Wiki",
    ) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "API-INTEGRATION" } });
    expect(
      document.querySelector<HTMLElement>('[data-wiki-card="api-integration"]')
        ?.style.display,
    ).toBe("");
  });
});

describe("WikiSearch 单挂（无卡片的极端壳）", () => {
  it("空关键词不炸、空态判定不误报", () => {
    render(<WikiSearch placeholder="搜" />);
    const input = screen.getByPlaceholderText("搜") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "任意" } });
    // 页面上没有卡也没有 #wiki-filter-empty——不抛错即过
    expect(input.value).toBe("任意");
  });
});
