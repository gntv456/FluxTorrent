import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { api } from "../lib/api-client";
import { appendVideoBlock } from "../lib/forum-video";
import { MarkdownRenderer } from "../components/forum-markdown";
import { TorrentRefBlock } from "../components/forum-torrent-card";
import { torrentLine } from "../lib/forum-torrent-line";
import { TorrentInsertRow } from "../components/forum-torrent-insert";

afterEach(() => {
  vi.restoreAllMocks();
});

const CARD = {
  id: 42,
  name: "星际穿越 IMAX 版",
  small_descr: "2014 4K HDR",
  size: 59441111040,
  seeders: 7,
  leechers: 1,
  promotion: "free",
  poster: null,
};

describe("torrentLine 识别口径", () => {
  it("认显式语法、站内相对链与绝对 URL 裸链", () => {
    expect(torrentLine("!torrent(42)")).toBe(42);
    expect(torrentLine("  !torrent(42)  ")).toBe(42);
    expect(torrentLine("/torrent/42")).toBe(42);
    expect(torrentLine("/torrent/42/")).toBe(42);
    expect(torrentLine("https://example.com/torrent/42")).toBe(42);
  });

  it("拒绝非法形态：非数字/空/带查询串/多段路径", () => {
    expect(torrentLine("!torrent(abc)")).toBeNull();
    expect(torrentLine("!torrent()")).toBeNull();
    expect(torrentLine("/torrent/42?x=1")).toBeNull();
    expect(torrentLine("/torrents/42")).toBeNull();
    expect(torrentLine("看我这个 /torrent/42")).toBeNull();
    expect(torrentLine("正文 !torrent(42)")).toBeNull();
  });
});

describe("forum markdown torrent blocks", () => {
  // 卡片取数有模块级缓存（cardCache），用例顺序即缓存写入顺序：
  // 渲染用例必须先于降级用例跑（vitest describe 按声明序执行）。
  it("把 !torrent(id) 独立行渲染为引用卡", async () => {
    vi.spyOn(api, "get").mockResolvedValue(CARD);
    render(<MarkdownRenderer source={`看看这个\n!torrent(42)`} />);
    expect(
      await screen.findByRole("link", { name: /星际穿越/ }),
    ).toHaveAttribute("href", "/torrent/42");
  });

  it("站内种子裸链 unfurl 成卡（不被视频候选抢走）", async () => {
    render(<MarkdownRenderer source={`看这个\n/torrent/42`} />);
    expect(
      await screen.findByRole("link", { name: /星际穿越/ }),
    ).toHaveAttribute("href", "/torrent/42");
  });

  it("代码围栏内不解析，显成字面量", () => {
    const { container } = render(
      <MarkdownRenderer source={"```\n!torrent(42)\n```"} />,
    );
    expect(container.textContent).toContain("!torrent(42)");
  });

  it("非法语法 !torrent(abc) 不成卡、按普通段落渲染", () => {
    const { container } = render(
      <MarkdownRenderer source="!torrent(abc)" />,
    );
    expect(container.textContent).toContain("!torrent(abc)");
  });
});

describe("TorrentRefBlock 降级链", () => {
  it("取数失败降级为失效框（fail-closed）", async () => {
    vi.spyOn(api, "get").mockRejectedValue(new Error("404"));
    render(<TorrentRefBlock id={999} />);
    await waitFor(() =>
      expect(screen.getByText("种子不存在或不可见")).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("link", { name: /种子不存在或不可见/ }),
    ).toHaveAttribute("href", "/torrent/999");
  });
});

describe("TorrentInsertRow 搜索选择器", () => {
  it("输入防抖后出候选，点选回调 !torrent(id) 并清空输入", async () => {
    vi.useFakeTimers();
    const get = vi
      .spyOn(api, "get")
      .mockResolvedValue({ items: [{ ...CARD, seeders: 3 }] });
    const onInsert = vi.fn();
    render(<TorrentInsertRow onInsert={onInsert} />);
    fireEvent.change(screen.getByRole("combobox"), {
      target: { value: "星际" },
    });
    vi.advanceTimersByTime(350);
    await vi.waitFor(() =>
      expect(get).toHaveBeenCalledWith(
        "/api/v1/torrents/suggest?q=%E6%98%9F%E9%99%85",
      ),
    );
    await vi.waitFor(() =>
      expect(screen.getByRole("option", { name: /星际穿越/ })).toBeVisible(),
    );
    fireEvent.click(screen.getByRole("option", { name: /星际穿越/ }));
    expect(onInsert).toHaveBeenCalledWith("!torrent(42)");
    expect(screen.getByRole("combobox")).toHaveValue("");
    vi.useRealTimers();
  });

  it("插行走 appendVideoBlock（空行分隔追加，与视频同契约）", () => {
    expect(appendVideoBlock("正文", "!torrent(42)")).toBe(
      "正文\n\n!torrent(42)\n",
    );
  });
});
