import { describe, it, expect } from "vitest";
import { render, screen, within } from "@testing-library/react";

import {
  ForumSideColumn,
  ForumUserCardBlock,
} from "@/components/forum-home-aside";
import { zhCN } from "@/i18n/zh-CN";
import type { ForumSide } from "@fluxtorrent/domain-types";
import type { ForumUserCard } from "@/lib/data-forum";

/**
 * 论坛首页两翼（左列用户卡 / 右列四块）。
 *
 * 钉的都是「读失败与没数据不许混」那一类：游客、取档失败、真的空
 * 三档各说各话；三档若都渲染成空列表，界面就在替站点撒谎。
 */

const dict = zhCN;
const t = dict.forums3;
const CUR = "魔力";

const me: ForumUserCard = {
  username: "青柠",
  class_name: "Power 动力",
  avatar_url: null,
  uploaded: 32100,
  downloaded: 10000,
  seeding: 26,
  spark: 420,
};

const HOT_A = {
  id: 419,
  title: "做种时 DHT 一定要关吗？",
  forum_name: "技术讨论",
  replies: 53,
};
const HOT_B = {
  id: 7,
  title: "本周水楼",
  forum_name: "灌水乐园",
  replies: 2,
};

const side: ForumSide = {
  stats: { topics: 74, posts: 126, today_topics: 3 },
  hot: [HOT_A, HOT_B],
  latest_replies: [
    {
      id: 900,
      topic_id: 419,
      topic_title: HOT_A.title,
      forum_name: HOT_A.forum_name,
      username: "流云",
      created_at: new Date(Date.now() - 5 * 60_000).toISOString(),
    },
  ],
};

const emptySide: ForumSide = {
  stats: { topics: 0, posts: 0, today_topics: 0 },
  hot: [],
  latest_replies: [],
};

function mountCard(u: typeof me | null) {
  return render(<ForumUserCardBlock me={u} dict={dict} currency={CUR} />);
}

function mountSide(state: "guest" | "ok" | "fail", s: ForumSide | null) {
  return render(<ForumSideColumn side={s} state={state} dict={dict} />);
}

describe("论坛首页左列用户卡", () => {
  it("三档数字取自 props（分享率算出来，不是写死）", () => {
    mountCard(me);
    expect(screen.getByText("3.21")).toBeInTheDocument();
    expect(screen.getByText(String(me.spark))).toBeInTheDocument();
    expect(screen.getByText(String(me.seeding))).toBeInTheDocument();
    expect(screen.getByText(me.class_name as string)).toBeInTheDocument();
  });

  it("魔力取档失败显「—」，不演成 0", () => {
    mountCard({ ...me, spark: null });
    expect(screen.getByText("—")).toBeInTheDocument();
    expect(screen.queryByText("0")).toBeNull();
  });

  it("游客：一个 0 值都不摆，给登录引导", () => {
    mountCard(null);
    expect(screen.queryByText("3.21")).toBeNull();
    expect(screen.queryByText("0")).toBeNull();
    const link = screen.getByRole("link");
    expect(link.getAttribute("href")).toBe("/login");
    expect(link.textContent).toContain("魔力");
  });
});

describe("论坛首页右列四块", () => {
  it("ok：热帖逐条成链，指向各自主题", () => {
    const { container } = mountSide("ok", side);
    const hot = container.querySelector(".fh-hot");
    expect(hot).not.toBeNull();
    const hrefs = within(hot as HTMLElement)
      .getAllByRole("link")
      .map((a) => a.getAttribute("href"));
    expect(hrefs).toEqual(["/forums/topic/419", "/forums/topic/7"]);
    expect(screen.getByText("74")).toBeInTheDocument();
    expect(
      screen.getByText(t.replyN.replace("{n}", "53")),
    ).toBeInTheDocument();
  });

  it("ok 且真的没有热帖：这才允许说「最近没有帖子在讨论」", () => {
    mountSide("ok", emptySide);
    expect(screen.getByText(t.emptyHot)).toBeInTheDocument();
  });

  it("guest：说「需登录」，不把 0 冒充统计", () => {
    mountSide("guest", null);
    expect(screen.getByText(t.guestTitle)).toBeInTheDocument();
    expect(screen.queryByText(t.statTopics)).toBeNull();
  });

  it("fail：说出取档失败，不与游客同框", () => {
    mountSide("fail", null);
    expect(screen.getByText(t.sideFailTitle)).toBeInTheDocument();
    expect(screen.queryByText(t.guestTitle)).toBeNull();
  });

  it("新手必读是静态块：三档都在（登录与否都得给引导）", () => {
    for (const st of ["guest", "ok", "fail"] as const) {
      const { unmount } = mountSide(st, st === "ok" ? side : null);
      expect(screen.getByText(t.guideTitle)).toBeInTheDocument();
      // CTA 文案是「发新主题」（i18n guideCta + 样图 f3b-guide 一致），
      // 落点是真发帖页 /forums/new——早前这里断言 /rules，与文案自相矛盾。
      expect(screen.getByRole("link", { name: t.guideCta })).toHaveAttribute(
        "href",
        "/forums/new",
      );
      unmount();
    }
  });
});
