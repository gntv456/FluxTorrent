import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { hasBBCode, renderBBCode } from "../lib/bbcode";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** [img] 走 ZoomImage（客户端件，读字典取 aria-label）⇒ 必须挂在
 *  LocaleProvider 里渲染。 */
function renderText(text: string): string {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <div data-testid="bb">{renderBBCode(text)}</div>
    </LocaleProvider>,
  ).getByTestId("bb").textContent;
}

function renderBB(text: string): HTMLElement {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <div>{renderBBCode(text)}</div>
    </LocaleProvider>,
  ).container;
}

describe("bbcode renderer", () => {
  it("detects bbcode presence", () => {
    expect(hasBBCode("[b]bold[/b]")).toBe(true);
    expect(hasBBCode("plain markdown # title")).toBe(false);
  });

  it("renders nesting and inline tags", () => {
    expect(renderText("[b]bold [i]both[/i][/b] tail")).toBe("bold both tail");
  });

  it("keeps [code] content raw (tags inside not parsed)", () => {
    expect(renderText("[code][b]not bold[/b][/code]")).toBe("[b]not bold[/b]");
  });

  it("rejects javascript: urls in [url] and [img]", () => {
    const html = renderBB("[url=javascript:alert(1)]x[/url]");
    expect(html.querySelector("a[href='javascript:alert(1)']")).toBeNull();
    const html2 = renderBB("[img]javascript:alert(1)[/img]");
    expect(html2.querySelector("img")).toBeNull();
  });

  it("tolerates unclosed tags (literal fallback, no crash)", () => {
    expect(renderText("a [b]b [i]c")).toBe("a b c");
  });

  it("renders real-world descr (poster img + MediaInfo quote + url)", () => {
    // 线上真实样例（走马上任 2026）：作品页曾按纯文本裸奔整段 BBCode
    const src = [
      "[quote][color=red][size=4][b]禁转PTTime[/b][/size][/color][/quote]",
      "[img]https://img3.pixhost.cc/p.jpg[/img]",
      "◎IMDb链接  [url=https://www.imdb.com/title/tt34423706/]imdb[/url]",
      "[quote]Complete name : x.mkv[/quote]",
    ].join("\n");
    const html = renderBB(src);
    expect(html.querySelectorAll("img")).toHaveLength(1);
    const imdb = "a[href='https://www.imdb.com/title/tt34423706/']";
    expect(html.querySelector(imdb)).not.toBeNull();
    expect(html.querySelectorAll("blockquote")).toHaveLength(2);
    // 渲染后不该再露出任何裸标签
    expect(html.textContent).not.toMatch(/\[(?:img|url|quote|b)\]/);
  });
});
