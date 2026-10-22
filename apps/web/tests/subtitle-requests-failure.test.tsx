import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { SubtitleRequestPanel } from "@/components/subtitle-request-panel";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-613：求字幕悬赏列表取数失败时，旧代码只弹一条几秒后消失的 toast，
 * 随后表格显示「暂无求字幕」——故障被演成空板（与 P-314 同形）。
 * 现在失败那一行常驻「加载失败」，真实空列表仍显示「暂无求字幕」。
 *
 * 负控：回放 HEAD subtitle-request-panel ⇒ 第一条红在「暂无求字幕仍在」，
 * 后两条正控两边都绿。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post: vi.fn() } };
});

const row = {
  id: 9,
  username: "someone",
  torrent_id: null,
  lang: "chs",
  descr: null,
  bounty: 150,
  contributors: [],
  status: 0,
  fulfilled_subtitle_id: null,
  created_at: "2026-10-20T00:00:00Z",
  claimed_by: null,
  deadline_at: null,
  deliver_at: null,
  offer_free: false,
  free_days: 0,
  crew: [],
};

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <SubtitleRequestPanel langs={[]} onMsg={() => {}} reloadKey={0} />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => ({
      json: async () => ({ data: {} }),
    })) as unknown as typeof fetch,
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function stubRequests(result: () => Promise<unknown>) {
  get.mockImplementation(async (path: string) => {
    if (path === "/api/v1/me") return { id: 7, username: "me" };
    if (path.startsWith("/api/v1/subtitles/requests")) return result();
    throw new Error(`unexpected ${path}`);
  });
}

describe("求字幕列表取数失败（P-613）", () => {
  it("失败显示加载失败，不再显示暂无求字幕", async () => {
    stubRequests(() =>
      Promise.reject(new ApiError(5000, "服务暂时不可用", null, 500, "t")),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.subtitles.reqEmpty)).toBeNull();
  });

  it("正控：真实空列表仍显示暂无求字幕", async () => {
    stubRequests(() => Promise.resolve([]));
    mount();
    expect(await screen.findByText(zhCN.subtitles.reqEmpty)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });

  it("正控：成功时正常渲染悬赏行", async () => {
    stubRequests(() => Promise.resolve([row]));
    mount();
    expect(await screen.findByText("chs")).toBeTruthy();
    expect(screen.queryByText(zhCN.subtitles.reqEmpty)).toBeNull();
  });
});
