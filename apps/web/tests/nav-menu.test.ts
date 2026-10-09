import { describe, expect, it } from "vitest";
import {
  buildNav,
  customNav,
  defaultNav,
  type MenuItem,
} from "@/lib/nav-menu";

/** 导航配置单源（M2）快照测试：桌面/抽屉/底 Tab 三处共用这份构建。
 *  变更分组结构/条目顺序是有意为之的设计决策——改这里须同步概念稿 §2。 */

const nav = {
  home: "首页",
  library: "资源库",
  official: "官种",
  forums: "论坛",
  messages: "消息",
  textbooks: "课本",
  medals: "我的勋章",
  top: "排行",
  magicPool: "站免池",
  myhr: "我的H&R",
  sparkLedger: "魔力明细",
  classes: "等级要求",
  contests: "大赛",
  medalWall: "勋章墙",
  frames: "头像挂件",
  gomoku: "五子棋",
  faq: "FAQ",
  donate: "捐赠",
  games: "娱乐屋",
  farm: "农场",
  dressup: "装扮",
  tasks: "任务",
  bank: "魔力银行",
  invites: "邀请",
  subtitles: "字幕",
  friends: "社交",
  requests: "求种",
  offers: "候选",
  jixiao: "绩效",
  preserve: "保种区",
  endangered: "濒危预警",
  teams: "保种协作",
  networks: "出品方",
  upload: "发布",
  exams: "我的考核",
  achievements: "成就墙",
  resurrections: "复活任务",
  discover: "发现",
  spark: "{magic}经济",
  growth: "成长荣誉",
  fun: "娱乐",
  more: "更多",
};
const tabbar = { shop: "商店" };
const allOn = () => true;

describe("defaultNav", () => {
  it("全模块开启时一级 5 项 + 5 分组，顺序稳定（快照）", () => {
    const r = defaultNav({
      nav,
      tabbar,
      currency: "魔力",
      modules: allOn,
      customItems: [],
    });
    expect(r.primary.map((p) => p.href)).toEqual([
      "/",
      "/torrents",
      "/forums",
      "/top",
      "/upload",
    ]);
    expect(r.groups.map((g) => g.group)).toEqual([
      "发现",
      "魔力经济",
      "成长荣誉",
      "娱乐",
      "更多",
    ]);
    // {magic} 占位替换：分组名与条目名都生效
    expect(r.groups[1].group).toBe("魔力经济");
    // 魔力经济组末两项：魔力总览 / 魔力明细（0226 C5 新增）
    expect(r.groups[1].items.at(-2)?.label).toBe("魔力经济");
    expect(r.groups[1].items.at(-1)?.label).toBe("魔力明细");
    // 成长荣誉组首项：等级要求公开页（0226 P2 触点 #2）
    expect(r.groups[2].items[0]?.href).toBe("/classes");
  });

  it("模块开关过滤：forums 关闭一级消失，games 关闭娱乐组剔除", () => {
    const r = defaultNav({
      nav,
      tabbar,
      currency: "魔力",
      modules: (k) => !(k === "forums" || k === "games"),
      customItems: [],
    });
    expect(r.primary.some((p) => p.href === "/forums")).toBe(false);
    const fun = r.groups.find((g) => g.group === "娱乐")!;
    expect(fun.items.some((i) => i.href === "/games")).toBe(false);
  });

  it("维度开关：network 维度在 → 出品方入口出现；缺 dims → 不出现", () => {
    const on = defaultNav({
      nav,
      tabbar,
      currency: "魔力",
      modules: allOn,
      dims: (k) => k === "network",
      customItems: [],
    });
    expect(
      on.groups
        .flatMap((g) => g.items)
        .some((i) => i.href === "/networks"),
    ).toBe(true);
    // 未传 dims：缺省全闭，不给空页面入口
    const off = defaultNav({
      nav,
      tabbar,
      currency: "魔力",
      modules: allOn,
      customItems: [],
    });
    expect(
      off.groups
        .flatMap((g) => g.items)
        .some((i) => i.href === "/networks"),
    ).toBe(false);
  });
});

describe("customNav", () => {
  const items = [
    { id: 1, parent_id: 0, label: "自A", url: "/a", location: "topbar" },
    { id: 2, parent_id: 1, label: "子1", url: "/a/1", location: "topbar" },
  ] as unknown as MenuItem[];

  it("自定义生效：一级取 top 项、子项成组", () => {
    const r = customNav(items);
    expect(r.active).toBe(true);
    expect(r.primary).toEqual([{ href: "/a", label: "自A" }]);
    expect(r.groups).toEqual([
      { group: "自A", items: [{ href: "/a/1", label: "子1" }] },
    ]);
  });

  it("buildNav：自定义优先、空则回退默认", () => {
    const custom = buildNav({
      nav,
      tabbar,
      currency: "魔力",
      modules: allOn,
      customItems: items,
    });
    expect(custom.primary[0].label).toBe("自A");
    const fallback = buildNav({
      nav,
      tabbar,
      currency: "魔力",
      modules: allOn,
      customItems: [],
    });
    expect(fallback.primary[0].label).toBe("首页");
  });
});
