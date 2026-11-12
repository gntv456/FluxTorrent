import { describe, it, expect, beforeAll, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TorrentSectionTabs } from "@/components/torrent-section-tabs";

/** 标签组验收（2026-11-13 省高改版）：桌面 tab 切换 + 懒挂载 + 旧锚点
 *  hash 兼容 + ≤768 平铺回落（FoldMobile 形态）。媒体查询打桩口径
 *  同 fold-anchor.test.tsx。 */

function stubMedia(compact: boolean) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: (q: string) => ({
      matches: compact ? /7(6[78])/.test(q) : false,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
    }),
  });
}

const items = [
  { id: "td-files", label: "文件列表", count: 5, node: <b>文件内容</b> },
  { id: "td-sec-subtitles", label: "字幕", node: <b>字幕内容</b> },
  { id: "td-sec-comments", label: "评论", count: 2, node: <b>评论内容</b> },
];

beforeAll(() => {
  stubMedia(false);
});

describe("TorrentSectionTabs（桌面态）", () => {
  it("tab 条渲染 label + 计数徽标；首 tab 激活且内容挂载", () => {
    render(<TorrentSectionTabs items={items} navLabel="页面导航" />);
    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(3);
    expect(tabs[0].textContent).toBe("文件列表5");
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    // 懒挂载：首 tab（初始激活）内容在，其余 tab 内容未挂载
    expect(screen.getByText("文件内容")).toBeTruthy();
    expect(screen.queryByText("字幕内容")).toBeNull();
    expect(screen.queryByText("评论内容")).toBeNull();
  });

  it("点击切换：目标挂载、旧内容隐藏不卸载（状态保留语义）", async () => {
    render(<TorrentSectionTabs items={items} navLabel="页面导航" />);
    await userEvent.click(screen.getByRole("tab", { name: "字幕" }));
    expect(screen.getByText("字幕内容")).toBeTruthy();
    // 文件 panel 隐藏但已访问过：仍挂载（hidden 属性，非卸载）
    const filesPanel = document.getElementById("td-files");
    expect(filesPanel?.hidden).toBe(true);
    expect(filesPanel?.textContent).toContain("文件内容");
  });

  it("旧锚点兼容：hash 命中即切 tab 并挂载", async () => {
    location.hash = "#td-sec-comments";
    render(<TorrentSectionTabs items={items} navLabel="页面导航" />);
    // 首帧 active=comments（初始 hash 生效）
    expect(screen.getByText("评论内容")).toBeTruthy();
    location.hash = "";
  });
});

describe("TorrentSectionTabs（移动态回落）", () => {
  it("≤768 不渲染 tab 条，各段回落 FoldMobile 折叠卡（id 落 details）", () => {
    stubMedia(true);
    render(<TorrentSectionTabs items={items} navLabel="页面导航" />);
    expect(screen.queryByRole("tab")).toBeNull();
    const details = document.querySelectorAll("details");
    expect(details).toHaveLength(3);
    expect(details[0].id).toBe("td-files");
    // 水合后按 M5.8 移动口径收起（FoldMobile open={!compact}）
    expect(details[0].open).toBe(false);
  });
});
