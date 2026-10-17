import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";

import { Tabs } from "@/components/ui/tabs";
import { FollowButton } from "@/components/forum-follow";
import { FeedLatestPanel } from "@/app/(main)/forums/feed/_parts/latest-panel";
import {
  MyRepliesPanel,
  MyTopicsPanel,
} from "@/app/(main)/forums/feed/_parts/mine-panels";
import { FollowSide } from "@/app/(main)/forums/feed/_parts/follow-side";
import { LocaleProvider } from "@/i18n/client";
import { api } from "@/lib/api-client";
import { zhCN } from "@/i18n/zh-CN";
import type { ForumMine } from "@fluxtorrent/domain-types";

/**
 * 关注流三标签 + 侧栏（样图 § forum-feed 补齐）。
 *
 * 钉的四件事：链接档标签渲染成真链接；三档状态（失败 / 真空 / 有数据）
 * 各说各话；侧栏已知关注态不再逐行探测；行链接用 id 而不是版块名。
 */

const dict = zhCN;
const t3 = dict.forums3;
const ITEMS = [
  { key: "latest", label: t3.tabFeed },
  { key: "mine", label: t3.tabMyTopics },
  { key: "replies", label: t3.tabMyReplies },
];
const HREFS = [
  "/forums/feed",
  "/forums/feed?tab=mine",
  "/forums/feed?tab=replies",
];

function withDict(ui: ReactNode) {
  return render(
    <LocaleProvider dict={dict} locale="zh-CN">
      {ui}
    </LocaleProvider>,
  );
}

const topicRow = {
  id: 419,
  title: "做种时 DHT 一定要关吗？",
  forum_id: 4,
  forum_name: "播种帮助",
  created_at: "2026-10-17T02:00:00Z",
  last_post_at: "2026-10-17T03:00:00Z",
  replies: 53,
  my_replies: 2,
};
const replyRow = {
  id: 900,
  topic_id: 419,
  topic_title: "做种时 DHT 一定要关吗？",
  forum_id: 4,
  forum_name: "播种帮助",
  created_at: "2026-10-17T03:00:00Z",
  excerpt: "关，站内口径是私有种子不进 DHT",
};
const env = (
  type: "topics" | "replies",
  items: unknown[],
): ForumMine => {
  return { type, page: 1, per: 20, total: items.length, items } as ForumMine;
};

const hrefsOf = () =>
  screen.getAllByRole("link").map((el) => el.getAttribute("href"));

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Tabs 链接档", () => {
  it("渲染真链接、当前档带 aria-current，且不指 aria-controls", () => {
    withDict(
      <Tabs
        idPrefix="feed"
        label={t3.tabAria}
        items={ITEMS}
        value="mine"
        hrefs={HREFS}
      />,
    );
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((el) => el.tagName)).toEqual(["A", "A", "A"]);
    expect(tabs.map((el) => el.getAttribute("href"))).toEqual(HREFS);
    expect(tabs[1]).toHaveAttribute("aria-current", "page");
    expect(tabs[0]).not.toHaveAttribute("aria-current");
    // SSR 一次只渲染当前面板 ⇒ 指过去就是悬空 id，链接档刻意不指
    tabs.forEach((el) => expect(el).not.toHaveAttribute("aria-controls"));
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("回调档仍是按钮 + aria-controls（老调用方不受影响）", () => {
    withDict(
      <Tabs
        idPrefix="t2"
        label="x"
        items={ITEMS}
        value="mine"
        onChange={() => {}}
      />,
    );
    expect(screen.getAllByRole("tab")[1]).toHaveAttribute(
      "aria-controls",
      "panel-t2-mine",
    );
  });
});

describe("关注流「关注动态」面板", () => {
  it("取档失败说出「没取到」，不演成「没有动静」", () => {
    withDict(<FeedLatestPanel feed={null} dict={dict} locale="zh-CN" />);
    expect(screen.getByText(t3.failTitle)).toBeInTheDocument();
    expect(screen.queryByText(dict.forums.feedEmpty)).toBeNull();
  });

  it("真的没有关注动态时才说那句", () => {
    withDict(
      <FeedLatestPanel
        feed={{ items: [], next_before: null }}
        dict={dict}
        locale="zh-CN"
      />,
    );
    expect(screen.getByText(dict.forums.feedEmpty)).toBeInTheDocument();
  });
});

describe("我参与的主题 / 我的回复", () => {
  it("null → 失败态；空数组 → 空态；两档不同文案", () => {
    const a = withDict(
      <MyTopicsPanel data={null} dict={dict} locale="zh-CN" />,
    );
    expect(screen.getByText(t3.failTitle)).toBeInTheDocument();
    a.unmount();
    withDict(
      <MyTopicsPanel data={env("topics", [])} dict={dict} locale="zh-CN" />,
    );
    expect(screen.getByText(t3.mineEmptyTopics)).toBeInTheDocument();
  });

  it("主题行链到主题页与版块页，用的是 id 不是名字", () => {
    withDict(
      <MyTopicsPanel
        data={env("topics", [topicRow])}
        dict={dict}
        locale="zh-CN"
      />,
    );
    const hrefs = hrefsOf();
    expect(hrefs).toContain("/forums/topic/419");
    expect(hrefs).toContain("/forums/4");
    expect(hrefs.some((h) => (h ?? "").includes("播种"))).toBe(false);
    expect(
      screen.getByText(t3.myRepliesN.replace("{n}", "2")),
    ).toBeInTheDocument();
  });

  it("回复行链到主题页与版块页并带摘要", () => {
    withDict(
      <MyRepliesPanel
        data={env("replies", [replyRow])}
        dict={dict}
        locale="zh-CN"
      />,
    );
    expect(hrefsOf()).toEqual(["/forums/topic/419", "/forums/4"]);
    expect(screen.getByText(/私有种子不进 DHT/)).toBeInTheDocument();
  });

  it("回复列表为空时说「还没有回过帖」", () => {
    withDict(
      <MyRepliesPanel
        data={env("replies", [])}
        dict={dict}
        locale="zh-CN"
      />,
    );
    expect(screen.getByText(t3.mineEmptyReplies)).toBeInTheDocument();
  });
});

describe("关注流侧栏", () => {
  const mine = {
    forums: [{ id: 4, name: "播种帮助" }],
    users: [
      { id: 7, name: "流云" },
      { id: 8, name: "晚风" },
    ],
    topics: [],
  };

  it("两块列表按数据渲染，空的那块说空", () => {
    const { unmount } = withDict(<FollowSide mine={mine} dict={dict} />);
    expect(screen.getByText(t3.sideBoards)).toBeInTheDocument();
    expect(screen.getByText("播种帮助")).toBeInTheDocument();
    expect(screen.getAllByRole("link").length).toBe(3);
    unmount();
    withDict(
      <FollowSide
        mine={{ forums: [], users: mine.users, topics: [] }}
        dict={dict}
      />,
    );
    expect(screen.getByText(t3.sideEmptyBoards)).toBeInTheDocument();
  });

  it("initial 已知关注态：不再逐行打 status", async () => {
    const get = vi.spyOn(api, "get");
    withDict(
      <FollowButton
        targetType="forum"
        targetId={4}
        initial={{ following: true, followers: 0, is_self: false }}
      />,
    );
    expect(screen.getByText(dict.forums.following)).toBeInTheDocument();
    expect(get).not.toHaveBeenCalled();
  });

  it("没给 initial 时仍会去探测（正控：上一条不是恒真）", () => {
    const get = vi
      .spyOn(api, "get")
      .mockResolvedValue({
        following: false,
        followers: 0,
        is_self: false,
      } as never);
    withDict(<FollowButton targetType="forum" targetId={5} />);
    expect(get).toHaveBeenCalledTimes(1);
  });
});
