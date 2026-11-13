import { describe, expect, it, vi, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "node:fs";

import { Descr } from "@/components/torrent-detail-parts";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 简介正文限高 + 图片放大（2026-11-13 UI 反馈）：
 *  「内容简介高度太矮看不到内容」+「简介里的图没限制大小，还得点开放大」。
 *  断言分三层：渲染层（结构/交互）、样式层（限高与缩略图上限写死在 CSS 里，
 *  jsdom 无排版引擎，只能查样式表原文）、负控（不溢出就不该出按钮）。 */

const CSS = "app/styles/ui.css";

function readCss(): string {
  // vitest 运行在 apps/web 根
  return readFileSync(CSS, "utf8");
}

/** 让 jsdom 报出可分辨的高度：.rt__body 有 100px 可视高，
 *  .rt__inner 的内容高由 innerH 控制（用于制造溢出/不溢出两种情形）。 */
let innerH = 300;

beforeAll(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList?.contains("rt__body") ? 100 : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList?.contains("rt__inner") ? innerH : 0;
    },
  });
});

function renderDescr(text: string) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <Descr text={text} />
    </LocaleProvider>,
  );
}

describe("简介限高与展开（RichText）", () => {
  it("正文超限高时出「展开全部」，点开变「收起」且容器标记展开", async () => {
    innerH = 300;
    const { container } = renderDescr("正文");
    const btn = screen.getByRole("button", { name: /展开全部/ });
    expect(btn).toBeTruthy();
    expect(container.querySelector(".rt.is-open")).toBeNull();
    await userEvent.click(btn);
    expect(container.querySelector(".rt.is-open")).not.toBeNull();
    expect(
      screen.getByRole("button", { name: /收起/ }).textContent,
    ).toContain(zhCN.ui.collapseAll);
  });

  it("负控：正文不超限高时不出现展开按钮", () => {
    innerH = 100;
    renderDescr("短正文");
    expect(screen.queryByRole("button", { name: /展开全部/ })).toBeNull();
  });
});

describe("简介图缩略图与放大浮层（ZoomImage）", () => {
  it("图渲染成可点的缩略图按钮，点击弹浮层看原图", async () => {
    innerH = 100;
    renderDescr("[img]https://img.example.com/a.png[/img]");
    const thumb = screen.getByRole("button", { name: zhCN.ui.zoomImage });
    const img = thumb.querySelector("img");
    expect(img).not.toBeNull();
    expect(img?.getAttribute("src")).toBe("https://img.example.com/a.png");
    expect(screen.queryByRole("dialog")).toBeNull();
    await userEvent.click(thumb);
    const dlg = screen.getByRole("dialog");
    expect(dlg.getAttribute("aria-label")).toBe(zhCN.ui.imageViewer);
    expect(dlg.querySelector("img")?.getAttribute("src")).toBe(
      "https://img.example.com/a.png",
    );
    expect(
      dlg.querySelector("a[href='https://img.example.com/a.png']"),
    ).not.toBeNull();
  });

  it("ESC 关浮层；多图时计数与左右切换对上", async () => {
    innerH = 100;
    renderDescr(
      "[img]https://img.example.com/a.png[/img]\n" +
        "[img]https://img.example.com/b.png[/img]",
    );
    const thumbs = screen.getAllByRole("button", { name: zhCN.ui.zoomImage });
    expect(thumbs).toHaveLength(2);
    await userEvent.click(thumbs[0]);
    expect(screen.getByText("1 / 2")).toBeTruthy();
    await userEvent.click(
      screen.getByRole("button", { name: zhCN.ui.nextImage }),
    );
    expect(screen.getByText("2 / 2")).toBeTruthy();
    expect(
      screen.getByRole("dialog").querySelector("img")?.getAttribute("src"),
    ).toBe("https://img.example.com/b.png");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("样式层：限高与缩略图上限（用户反馈的数值口径）", () => {
  it("简介正文限高不再是 320px（改为 560px + 展开）", () => {
    const css = readCss();
    expect(css).toMatch(/\.rt__body\s*\{[^}]*max-height:\s*560px/);
    expect(css).not.toMatch(/\.rt__body\s*\{[^}]*max-height:\s*320px/);
  });

  it("简介图缩略图有上限（宽 420 / 高 260），不再原尺寸铺开", () => {
    const css = readCss();
    expect(css).toMatch(
      /\.rt-zoom__img\s*\{[^}]*max-width:\s*min\(100%,\s*420px\)/,
    );
    expect(css).toMatch(/\.rt-zoom__img\s*\{[^}]*max-height:\s*260px/);
  });
});
