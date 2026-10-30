import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

import {
  TorrentsSearchBox,
} from "@/app/(main)/torrents/_parts/torrents-search-box";
import {
  TorrentsViewSwitch,
} from "@/app/(main)/torrents/_parts/torrents-view-switch";
import {
  withParam,
  withoutCursor,
} from "@/app/(main)/torrents/_parts/torrents-utils";
import { buildTorrentChips } from "@/app/(main)/torrents/_parts/torrents-chips";
import { zhCN } from "@/i18n/zh-CN";

/** 渲染层接线判据：高级筛选项的 input name 必须与 page.tsx 转发的
 *  后端参数名逐字一致。旧实现由 rangeInputs 统一拼 `_min/_max`，实际提交
 *  `date_min` / `min_seeders_min` / `leechers_min` / `completed_min`，
 *  而页面只读 `date_from` / `min_seeders` / `min_leechers` /
 *  `min_completed`，用户填了等于没填。 */
describe("torrents 高级筛选表单字段名", () => {
  function fields(): string[] {
    const { container } = render(
      <TorrentsSearchBox
        dict={zhCN}
        sp={{}}
        categories={[{ id: 1, label: "电影" }]}
        tags={[]}
        tagGroups={[]}
        secDict={null}
        dimKinds={[]}
        selectedCats={new Set()}
        chips={[]}
        advancedOpen
        nonSearchChips={0}
        showImdb={false}
        withParam={() => "/torrents"}
      />,
    );
    return Array.from(
      container.querySelectorAll<HTMLInputElement>("input[name]"),
    ).map((el) => el.name);
  }

  it("数值/日期区间使用后端真实参数名", () => {
    const names = fields();
    for (const name of [
      "date_from",
      "date_to",
      "size_min",
      "size_max",
      "min_seeders",
      "max_seeders",
      "min_leechers",
      "max_leechers",
      "min_completed",
      "max_completed",
      "rating_min",
    ]) {
      expect(names).toContain(name);
    }
  });

  it("不再提交页面不认识的旧名", () => {
    const names = fields();
    for (const name of [
      "date_min",
      "date_max",
      "min_seeders_min",
      "min_seeders_max",
      "leechers_min",
      "leechers_max",
      "completed_min",
      "completed_max",
    ]) {
      expect(names).not.toContain(name);
    }
  });

  it("提交搜索时保留编排/视图/每页条数", () => {
    const { container } = render(
      <TorrentsSearchBox
        dict={zhCN}
        sp={{ group: "1", view: "poster", limit: "50" }}
        categories={[{ id: 1, label: "电影" }]}
        tags={[]}
        tagGroups={[]}
        secDict={null}
        dimKinds={[]}
        selectedCats={new Set()}
        chips={[{ id: "search", key: "search", text: "x", href: "/torrents" }]}
        advancedOpen
        nonSearchChips={0}
        showImdb={false}
        withParam={() => "/torrents"}
      />,
    );
    const value = (name: string) =>
      container.querySelector<HTMLInputElement>(
        `input[name="${name}"]`,
      )?.value;
    expect(value("group")).toBe("1");
    expect(value("view")).toBe("poster");
    expect(value("limit")).toBe("50");
    const clear = container.querySelector("a.tsb-active__clear");
    expect(clear?.getAttribute("href")).toContain("group=1");
    expect(clear?.getAttribute("href")).toContain("view=poster");
    expect(clear?.getAttribute("href")).toContain("limit=50");
    expect(clear?.getAttribute("href")).not.toContain("search=");
  });

  it("换排序/筛选时丢弃旧游标与页码，保留其余筛选", () => {
    expect(
      withoutCursor({
        cursor: "abc~1",
        dir: "prev",
        p: "3",
        sort: "name",
        search: "x",
        group: "1",
      }),
    ).toEqual({ sort: "name", search: "x", group: "1" });
  });

  it("移除任一筛选 chip 都不保留旧游标", () => {
    const chips = buildTorrentChips({
      sp: {
        cursor: "old~1",
        p: "2",
        category_id: "1,2",
        tag_ids: "3",
        tag_mode: "all",
        sec_media: "4",
        date_from: "2026-01-01",
      },
      dict: zhCN,
      categories: [
        { id: 1, label: "电影" },
        { id: 2, label: "剧集" },
      ],
      tags: [{ id: 3, name: "标签", kind: "attribute" }],
      secDict: {
        media: [{ id: 4, kind: "media", name: "媒介", sort: 0 }],
      },
      dimKinds: [{ kind: "media", label: "媒介", sort: 0 }],
      withParam,
    });
    expect(chips.length).toBeGreaterThan(0);
    for (const c of chips) {
      expect(c.href).not.toContain("cursor=");
      expect(c.href).not.toContain("p=");
      expect(c.href).not.toContain("dir=");
    }
  });

  it("视图/条数控件显式写 URL，不被用户偏好回落吃掉", () => {
    const { container } = render(
      <TorrentsViewSwitch
        dict={zhCN}
        sp={{ cursor: "abc~1", p: "2" }}
        view="card"
        grouped={false}
        pageSize={20}
        withParam={withParam}
      />,
    );
    const hrefs = Array.from(
      container.querySelectorAll<HTMLAnchorElement>("a[href]"),
    ).map((a) => a.getAttribute("href") ?? "");
    expect(hrefs.some((h) => h.includes("view=table"))).toBe(true);
    expect(hrefs.some((h) => h.includes("limit=20"))).toBe(true);
    expect(hrefs.every((h) => !h.includes("cursor="))).toBe(true);
  });
});
