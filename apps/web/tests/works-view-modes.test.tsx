import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

/**
 * 作品聚合三形态与 Colapt 三档一一对应（2026-10-31）：
 *   列表 = 宽松（每部作品一条媒体行 + 七列种子表）
 *   卡片 = 网格（2~3 列作品卡，各挂前两条种子 + 「其余 N 个版本」）
 *   海报墙 = 瀑布（封面墙，点封面就地展开版本面板）
 *
 * 这三条互为负控：任一形态串到别的组件上，对应断言里的 querySelector
 * 就会同时命中两个选择器（例如列表里出现 .wk-grid），当场报红。
 */

const state = vi.hoisted(() => ({ items: [] as unknown[] }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), refresh: vi.fn() }),
}));

vi.mock("@/lib/api-client", () => ({
  api: {
    get: vi.fn(async () => ({
      items: state.items,
      total: state.items.length,
      limit: 20,
      page: 1,
    })),
  },
  ApiError: class ApiError extends Error {},
}));

import { TorrentWorks } from "@/app/(main)/torrents/_parts/torrents-works";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";
import type {
  TorrentListItem,
  WorkWithSeeds,
} from "@fluxtorrent/domain-types";

function seed(
  id: number,
  name: string,
  official = true,
  shots: string[] = [],
  ends: string | null = null,
): TorrentListItem {
  return {
    id,
    info_hash: "a".repeat(40),
    name,
    small_descr: null,
    category_id: 1,
    medium_id: null,
    grade_id: null,
    edition_id: null,
    size: 3_221_225_472,
    seeders: 6,
    leechers: 0,
    times_completed: 13,
    comments: 0,
    official,
    anonymous: false,
    approval_status: 1,
    sticky: false,
    promotion: "free",
    promotion_ends_at: ends,
    rating: null,
    poster: null,
    imdb_id: null,
    screenshots: shots,
    owner_name: null,
    created_at: "2026-10-30T12:00:00Z",
    tags: [],
    sec_names: [],
  };
}

function work(): WorkWithSeeds {
  return {
    key: "n0123456789abcdef0123456789abcdef",
    name: "尚公主 | 2026 2160p WEB-DL",
    poster: null,
    rating: "5.7",
    category_id: 1,
    synopsis: null,
    detail: null,
    imdb_id: "tt0000001",
    tmdb_id: null,
    tmdb_url: null,
    douban_id: null,
    meta: {
      source: "tmdb",
      name: "尚公主",
      year: 2026,
      rating: 9.5,
      runtime: 45,
      region: "中国大陆",
      director: ["秦榛"],
      cast_list: ["孟子义"],
      genres: ["剧情", "爱情", "古装"],
      poster: null,
      synopsis: null,
    },
    year: 2026,
    torrents: 4,
    seeders: 6,
    leechers: 0,
    size: 9_663_676_416,
    latest_at: "2026-10-30T12:00:00Z",
    top_id: 101,
    tags: [],
    seeds: [
      seed(101, "Tigers.S01E18.2160p.WEB-DL.H.265", true, [
        "https://img.example/a.jpg",
        "https://img.example/b.jpg",
      ], "2099-01-01T00:00:00Z"),
      seed(102, "Tigers.S01E17.2160p.WEB-DL.H.265"),
      seed(103, "Tigers.S01E16.2160p.WEB-DL.H.265", false),
    ],
  };
}

async function view(
  mode: "table" | "card" | "poster",
  workFields: string[] = [],
): Promise<HTMLElement> {
  const node = await TorrentWorks({
    dict: zhCN,
    locale: "zh-CN",
    sp: {},
    cats: [{ id: 1, label: "电影" }],
    view: mode,
    perPage: 20,
    withParam: () => "/torrents",
    workFields,
  });
  const { container } = render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {node}
    </LocaleProvider>,
  );
  return container;
}

beforeEach(() => {
  state.items = [work()];
});

describe("作品聚合三形态", () => {
  it("列表 = 宽松：媒体行 + 七列种子表（时间/下载/完成都在）", async () => {
    const c = await view("table");
    expect(c.querySelector(".wk-card")).not.toBeNull();
    expect(c.querySelector(".wk-grid")).toBeNull();
    expect(c.querySelector(".wk-wall")).toBeNull();
    const head = c.querySelector(".wk-shead");
    expect(head).not.toBeNull();
    expect(head!.querySelector(".wk-time")).not.toBeNull();
    expect(head!.querySelector(".wk-leech")).not.toBeNull();
    expect(head!.querySelector(".wk-done")).not.toBeNull();
    expect(c.querySelectorAll(".wk-row")).toHaveLength(3);
    // 促销徽标带紧凑倒计时（2099 截止 → 末尾 D）
    expect(
      c.querySelector(".wk-chip-left")!.textContent,
    ).toMatch(/^\d+D$/);
  });

  it("宽松行可就地展开截图与发布者，再点收起", async () => {
    const c = await view("table");
    expect(c.querySelector(".wk-seed-more")).toBeNull();
    await userEvent.click(c.querySelector(".wk-chev")!);
    const more = c.querySelector(".wk-seed-more");
    expect(more).not.toBeNull();
    expect(more!.textContent).toContain("发布者");
    expect(more!.querySelectorAll(".wk-shot img")).toHaveLength(2);
    await userEvent.click(c.querySelector(".wk-chev")!);
    expect(c.querySelector(".wk-seed-more")).toBeNull();
  });

  it("宽松卡的海报上不重复片名", async () => {
    const t = await view("table");
    expect(
      t.querySelector(".wk-poster")!.textContent,
    ).not.toContain("尚公主");
  });

  it("站长隐藏字段：标签/导演/简介/演员/评分可收起，标题保留", async () => {
    const c = await view("table", [
      "tags",
      "director",
      "synopsis",
      "cast",
      "ratings",
      "year",
    ]);
    expect(c.querySelector(".wk-tags")).toBeNull();
    expect(c.querySelector(".wk-syn")).toBeNull();
    expect(c.querySelector(".wk-crew")).toBeNull();
    expect(c.querySelector(".wk-head-ratings")).toBeNull();
    expect(c.querySelector(".wk-year")).toBeNull();
    expect(
      c.querySelector(".wk-media-meta")!.textContent,
    ).not.toContain("导演");
    expect(c.querySelector(".wk-title")!.textContent).toContain(
      "尚公主",
    );
  });

  it("卡片 = 网格：作品卡只挂前两条种子 + 其余 N 个版本", async () => {
    const c = await view("card");
    expect(c.querySelector(".wk-grid")).not.toBeNull();
    expect(c.querySelector(".wk-card")).toBeNull();
    expect(c.querySelectorAll(".wk-gcard")).toHaveLength(1);
    expect(c.querySelectorAll(".wk-mrow")).toHaveLength(2);
    expect(screen.getByText(/其余 2 个版本/)).toBeInTheDocument();
    // 网格卡不挂七列表头：那是「宽松」的形态
    expect(c.querySelector(".wk-shead")).toBeNull();
    // 封面不重复片名（片名在右侧）
    expect(
      c.querySelector(".wk-gposter")!.textContent,
    ).not.toContain("尚公主");
    // 迷你种子行的促销徽标同样带倒计时
    expect(
      c.querySelector(".wk-mrow .wk-chip-left")!.textContent,
    ).toMatch(/^\d+D$/);
  });

  it("海报墙 = 瀑布：点封面就地展开版本面板，关闭即收起", async () => {
    const c = await view("poster");
    expect(c.querySelector(".wk-wall")).not.toBeNull();
    expect(c.querySelector(".wk-card")).toBeNull();
    expect(c.querySelector(".wk-grid")).toBeNull();
    expect(c.querySelectorAll(".wk-wall-it")).toHaveLength(1);
    expect(c.querySelector(".wk-wall-panel")).toBeNull();
    // 封面不重复片名（片名在封面下方信息条）
    expect(
      c.querySelector(".wk-wall-poster")!.textContent,
    ).not.toContain("尚公主");
    await userEvent.click(screen.getByTitle("尚公主"));
    const panel = c.querySelector(".wk-wall-panel");
    expect(panel).not.toBeNull();
    // 面板按「官方版本」分组：只列官方种（2 条），其余 2 条进计数链接
    expect(panel!.textContent).toContain("官方版本");
    expect(panel!.querySelectorAll(".wk-mrow")).toHaveLength(2);
    expect(panel!.textContent).not.toContain("S01E16");
    expect(panel!.textContent).toContain("查看作品");
    expect(panel!.textContent).toContain("其余 2 个版本");
    // 面板内种子行与海报角标都带倒计时
    expect(
      panel!.querySelector(".wk-mrow .wk-chip-left")!.textContent,
    ).toMatch(/^\d+D$/);
    expect(
      c.querySelector(".wk-wall-promo .wk-chip-left")!.textContent,
    ).toMatch(/^\d+D$/);
    await userEvent.click(screen.getByLabelText("关闭预览"));
    expect(c.querySelector(".wk-wall-panel")).toBeNull();
  });
});
