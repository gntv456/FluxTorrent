import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { EmbedRule } from "../lib/data-forum";
import { api } from "../lib/api-client";
import {
  appendVideoBlock,
  isBilibiliShortUrl,
  splitVideoUrls,
} from "../lib/forum-video";
import { MarkdownRenderer } from "../components/forum-markdown";
import { VideoInsertRow } from "../components/forum-video-insert";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const BV1 = "https://www.bilibili.com/video/BV1GJ411x7h7";
const BV2 = "https://www.bilibili.com/video/BV1xx411c7mD";

const RULES: EmbedRule[] = [
  {
    id: 1,
    provider: "bilibili",
    name_zh: "哔哩哔哩",
    url_pattern:
      "^https?://(www\\.)?bilibili\\.com/video/(BV[0-9A-Za-z]+)",
    embed_template:
      "https://player.bilibili.com/player.html?bvid=$2",
    embed_origin: "https://player.bilibili.com/",
    render_kind: "iframe",
    aspect: "16:9",
    extra_params: null,
  },
];

describe("forum video helpers", () => {
  it("splits pasted links by whitespace/comma and dedupes", () => {
    expect(splitVideoUrls(` ${BV1}\n${BV2}, ${BV1} `)).toEqual([
      BV1,
      BV2,
    ]);
  });

  it("appends a video block with a blank line separator", () => {
    expect(appendVideoBlock("", "!video(x)")).toBe("!video(x)\n");
    expect(appendVideoBlock("正文\n", "!video(x)")).toBe(
      "正文\n\n!video(x)\n",
    );
    expect(appendVideoBlock("  \n", "!video(x)")).toBe("!video(x)\n");
  });

  it("recognises b23.tv short links only", () => {
    expect(isBilibiliShortUrl("https://b23.tv/ZPpraIl")).toBe(true);
    expect(isBilibiliShortUrl("https://www.bilibili.com/video/BV1")).toBe(
      false,
    );
  });
});

describe("forum markdown video blocks", () => {
  it("breaks a paragraph at !video glued to the previous line", () => {
    const { container } = render(
      <MarkdownRenderer
        source={`正文介绍\n!video(${BV1})`}
        embedRules={RULES}
      />,
    );
    expect(container.textContent).not.toContain("!video(");
    expect(
      screen.getByRole("button", { name: "点击播放" }),
    ).toBeInTheDocument();
  });

  it("turns a bare URL line after text into a player", () => {
    render(
      <MarkdownRenderer
        source={`看这个视频\n${BV1}`}
        embedRules={RULES}
      />,
    );
    expect(
      screen.getByRole("button", { name: "点击播放" }),
    ).toBeInTheDocument();
  });

  it("renders consecutive video lines as separate blocks", () => {
    render(
      <MarkdownRenderer
        source={`!video(${BV1})\n!video(${BV2})`}
        embedRules={RULES}
      />,
    );
    expect(screen.getAllByRole("button", { name: "点击播放" })).toHaveLength(
      2,
    );
  });

  it("keeps unmatched video syntax as a plain link", () => {
    render(
      <MarkdownRenderer
        source={`!video(https://example.com/v)`}
        embedRules={RULES}
      />,
    );
    expect(screen.getByRole("link")).toHaveAttribute(
      "href",
      "https://example.com/v",
    );
  });

  it("mounts the allowlisted iframe after clicking play", () => {
    render(<MarkdownRenderer source={BV1} embedRules={RULES} />);
    fireEvent.click(screen.getByRole("button", { name: "点击播放" }));
    expect(screen.getByTitle("哔哩哔哩")).toHaveAttribute(
      "src",
      "https://player.bilibili.com/player.html?bvid=BV1GJ411x7h7",
    );
  });

  it("resolves b23.tv short links before matching", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        json: async () => ({ url: BV1 }),
      })),
    );
    render(
      <MarkdownRenderer
        source="https://b23.tv/ZPpraIl"
        embedRules={RULES}
      />,
    );
    expect(
      await screen.findByRole("button", { name: "点击播放" }),
    ).toBeInTheDocument();
  });
});

describe("VideoInsertRow multi-link insert", () => {
  it("adds an input box and inserts every URL as !video lines", async () => {
    vi.spyOn(api, "get").mockResolvedValue(RULES);
    const onInsert = vi.fn();
    render(<VideoInsertRow onInsert={onInsert} />);
    expect(
      screen.getAllByPlaceholderText(/贴入视频页链接/),
    ).toHaveLength(1);
    fireEvent.click(
      screen.getByRole("button", { name: /添加视频/ }),
    );
    const boxes = screen.getAllByPlaceholderText(/贴入视频页链接/);
    expect(boxes).toHaveLength(2);
    fireEvent.change(boxes[0], { target: { value: BV1 } });
    fireEvent.change(boxes[1], { target: { value: BV2 } });
    const btn = screen.getByRole("button", { name: "插入" });
    await waitFor(() => expect(btn).toBeEnabled());
    fireEvent.click(btn);
    await waitFor(() =>
      expect(onInsert).toHaveBeenCalledWith(
        `!video(${BV1})\n!video(${BV2})`,
      ),
    );
    expect(
      screen.getAllByPlaceholderText(/贴入视频页链接/),
    ).toHaveLength(1);
  });
});
