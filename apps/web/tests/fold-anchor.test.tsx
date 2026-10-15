import { describe, it, expect, vi, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FoldMobile } from "@/components/fold-mobile";
import { TorrentAnchorBar } from "@/components/torrent-anchor";
import { zhCN } from "@/i18n/zh-CN";

/** M6.2 验收：折叠块角标/id 传递 + 锚点条 items 组装与跳转
 *  （设计稿第八章步骤 4「折叠块角标逻辑单测」）。 */

function stubMedia(compact: boolean) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: (q: string) => ({
      matches: compact ? q.includes("767") : false,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
    }),
  });
}

beforeAll(() => {
  // jsdom 无 IntersectionObserver / scrollIntoView：锚点条依赖，打桩
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
      takeRecords() {
        return [];
      }
    },
  );
  Element.prototype.scrollIntoView = vi.fn();
});

describe("FoldMobile 角标与锚点 id（M6.2）", () => {
  it("桌面态默认展开，title/count/children/id 全部落到 details 上", () => {
    stubMedia(false);
    render(
      <FoldMobile title="文件列表" count={5} id="td-files">
        <span>文件内容</span>
      </FoldMobile>,
    );
    const details = document.querySelector("details");
    expect(details).not.toBeNull();
    expect(details?.id).toBe("td-files");
    expect(details?.open).toBe(true);
    expect(screen.getByText("文件列表")).toBeTruthy();
    expect(screen.getByText("5")).toBeTruthy();
    expect(screen.getByText("文件内容")).toBeTruthy();
  });

  it("不传 id 时 details 无 id（锚点目标可选）", () => {
    stubMedia(false);
    render(
      <FoldMobile title="字幕">
        <span>x</span>
      </FoldMobile>,
    );
    expect(document.querySelector("details")?.id).toBe("");
  });
});

describe("TorrentAnchorBar（M6.2）", () => {
  it("桌面态渲染 null（导航角色由右信息栏承担）", () => {
    stubMedia(false);
    const { container } = render(
      <TorrentAnchorBar
        descr
        files={3}
        subtitles
        comments={2}
        dict={zhCN}
      />,
    );
    expect(container.querySelector(".td-anchor")).toBeNull();
  });

  it("移动态四 Tab：标题 + 计数徽标 + 占位符剥离", () => {
    stubMedia(true);
    render(
      <TorrentAnchorBar
        descr
        files={5}
        subtitles
        comments={1}
        dict={zhCN}
      />,
    );
    const nav = document.querySelector(".td-anchor");
    expect(nav?.getAttribute("aria-label")).toBe(zhCN.torrent.anchorNav);
    const tabs = screen.getAllByRole("button");
    expect(tabs).toHaveLength(4);
    expect(tabs[0].textContent).toBe(zhCN.torrent.descrTitle);
    expect(tabs[1].textContent).toBe(`${zhCN.torrent.filesTitle}5`);
    expect(tabs[2].textContent).toBe(zhCN.subtitles.panelTitle);
    // commentsTitle「评论 ({n})」→ 剥占位符 + 徽标
    expect(tabs[3].textContent).toBe("评论1");
  });

  it("null 计数的节不渲染 Tab（files=null/comments=null）", () => {
    stubMedia(true);
    render(
      <TorrentAnchorBar
        descr={false}
        files={null}
        subtitles
        comments={null}
        dict={zhCN}
      />,
    );
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  it("点击 Tab：折叠卡自动展开 + 平滑滚动 + 高亮跟随", async () => {
    stubMedia(true);
    render(
      <>
        <details id="td-files">
          <summary>文件列表</summary>
          <div>内容</div>
        </details>
        <TorrentAnchorBar
          descr={false}
          files={2}
          subtitles={false}
          comments={null}
          dict={zhCN}
        />
      </>,
    );
    const files = document.getElementById("td-files") as HTMLDetailsElement;
    expect(files.open).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: /文件列表/ }));
    expect(files.open).toBe(true);
    expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
    const on = document.querySelector(".td-anchor__tab.is-on");
    expect(on?.textContent).toContain(zhCN.torrent.filesTitle);
  });
});
