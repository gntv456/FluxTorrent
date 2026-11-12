import { describe, it, expect, beforeAll, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { zhCN } from "@/i18n/zh-CN";
import { TorrentSubtitles } from "@/components/torrent-subtitles";

/** 字幕面板发布改弹窗（2026-11-13）：
 *  标题行右挂「上传字幕」按钮 → ConfirmDialog 弹窗发布；
 *  内联表单退役（不再把表格顶下去）。 */

const rows = [
  {
    id: 7,
    torrent_id: 40,
    title: "简体中文字幕",
    lang: "chs",
    size: 12345,
    downloads: 3,
    username: "alice",
    created_at: "2026-11-01T00:00:00Z",
  },
];

// vi.mock 会被提升到文件顶部执行：mock 工厂内不能引用外部变量，
// 清单数据写死在工厂里
vi.mock("@/lib/api-client", () => ({
  api: {
    get: vi.fn().mockResolvedValue([
      {
        id: 7,
        torrent_id: 40,
        title: "简体中文字幕",
        lang: "chs",
        size: 12345,
        downloads: 3,
        username: "alice",
        created_at: "2026-11-01T00:00:00Z",
      },
    ]),
    post: vi.fn().mockResolvedValue({}),
  },
  ApiError: class extends Error {},
}));

vi.mock("@/i18n/client", () => ({
  // zhCN 真字典（工厂里 import 会再进 mock 链，直接内联消费键切片）
  useI18n: () => ({
    dict: zhCN,
    locale: "zh-CN",
  }),
  apiErrorMessage: () => "失败",
}));

function stubMedia(compact: boolean) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: (q: string) => ({
      matches: compact ? /76[0-9]/.test(q) : false,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }),
  });
}

beforeAll(() => {
  stubMedia(false);
});

describe("TorrentSubtitles 发布弹窗", () => {
  it("标题行：panelTitle + 计数 + 右挂上传按钮（bare 形态）", async () => {
    render(<TorrentSubtitles torrentId={40} imdbId={null} bare />);
    const head = document.querySelector(".td-sub-head");
    expect(head).not.toBeNull();
    expect(screen.getByText(zhCN.subtitles.panelTitle)).toBeTruthy();
    // 计数随清单加载出现（api mock resolve 后）
    await waitFor(() =>
      expect(screen.getByText("简体中文字幕")).toBeTruthy(),
    );
    expect(screen.getByText("1")).toBeTruthy();
  });

  it("点上传按钮弹窗打开；空表单点确认不关窗，提示必填", async () => {
    render(<TorrentSubtitles torrentId={40} imdbId={null} bare />);
    await userEvent.click(
      await screen.findByRole("button", {
        name: zhCN.subtitles.uploadTitle,
      }),
    );
    // 弹窗打开：文件/标题/语言三槽（zhCN 真键）
    expect(screen.getByText(zhCN.subtitles.file)).toBeTruthy();
    expect(screen.getByText(zhCN.subtitles.lang)).toBeTruthy();
    // 空表单点「上传文件」：弹窗不关，出必填提示
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.subtitles.uploadBtn }),
    );
    expect(document.querySelector(".ui-cfm")).not.toBeNull();
    expect(screen.getByText(/必须填写/)).toBeTruthy();
  });

  it("取消关窗：无内联表单残留（旧展开形态已退役）", async () => {
    render(<TorrentSubtitles torrentId={40} imdbId={null} bare />);
    await userEvent.click(
      await screen.findByRole("button", {
        name: zhCN.subtitles.uploadTitle,
      }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.common.cancel }),
    );
    expect(document.querySelector(".ui-cfm")).toBeNull();
    // 弹窗关闭后无 file input 残留在面板里
    expect(document.querySelector('input[type="file"]')).toBeNull();
  });
});
