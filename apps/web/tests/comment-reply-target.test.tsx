import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Comments } from "@/components/torrent-detail-blocks";
import { TorrentSocial } from "@/components/torrent-social";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";
import type { TorrentComment } from "@fluxtorrent/domain-types";

/**
 * 0156 楼中楼：点「回复」在一条**回复**上时，提交的 parent_id 必须是被
 * 回复的那条评论（后端据此归一到根楼层并写 reply_to_user）。此前界面传的是
 * 根楼层 id，后端看不到「回复的是谁」，`reply_to_user` 恒为 NULL，
 * 结果楼中楼里永远没有「回复 @xxx」标注（2026-10-21 现网 API 实测：
 * 传回复 id ⇒ reply_to_user='root'；传根 id ⇒ NULL）。
 */

const T = zhCN.torrent;
const COMMENTS: TorrentComment[] = [
  {
    id: 101,
    torrent_id: 7,
    username: "alice",
    body: "根评论",
    created_at: "2026-10-20T10:00:00Z",
    likes: 0,
    liked_by_me: false,
    parent_id: null,
    reply_to_user: null,
  },
  {
    id: 102,
    torrent_id: 7,
    username: "bob",
    body: "一楼回复",
    created_at: "2026-10-20T10:01:00Z",
    likes: 0,
    liked_by_me: false,
    parent_id: 101,
    reply_to_user: null,
  },
];

let sent: Record<string, unknown> | null = null;
const originalScroll = Element.prototype.scrollIntoView;

function res(body: unknown): Response {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => body,
  } as unknown as Response;
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <TorrentSocial torrentId={7} />
      <Comments
        comments={COMMENTS}
        torrentId={7}
        dict={zhCN}
        locale="zh-CN"
        total={COMMENTS.length}
      />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  sent = null;
  document.cookie = "flux.session=1; path=/";
  document.cookie = "flux_csrf=abc123; path=/";
  Element.prototype.scrollIntoView = vi.fn();
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (init?.method === "POST" && url.endsWith("/torrents/7/comments")) {
        sent = JSON.parse(String(init.body ?? "{}")) as Record<string, unknown>;
      }
      return res({
        code: 0,
        message: "ok",
        data: { id: 103 },
        request_id: "t",
      });
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  Element.prototype.scrollIntoView = originalScroll;
});

describe("评论楼中楼回复目标", () => {
  it("回复某条回复时，提交的是该回复的 id 而非根楼层 id", async () => {
    mount();
    const replyButtons = screen.getAllByRole("button", {
      name: T.replyBtn,
    });
    expect(replyButtons).toHaveLength(2);
    await userEvent.click(replyButtons[1]);
    // 横幅显示的目标必须与真正提交的目标一致
    expect(
      screen.getByText(T.replyTo.replace("{user}", "bob")),
    ).toBeTruthy();
    await userEvent.type(
      screen.getByPlaceholderText(T.commentPlaceholder),
      "回复 bob 的回复",
    );
    await userEvent.click(
      screen.getByRole("button", { name: T.commentSubmit }),
    );
    await waitFor(() => expect(sent).not.toBeNull());
    expect(sent).toEqual({ body: "回复 bob 的回复", parent_id: 102 });
  });
});
