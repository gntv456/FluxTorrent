import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { hasBBCode, renderBBCode } from "../lib/bbcode";

function renderText(text: string): string {
  return render(<div data-testid="bb">{renderBBCode(text)}</div>).getByTestId(
    "bb",
  ).textContent;
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
    const html = render(
      <div>{renderBBCode("[url=javascript:alert(1)]x[/url]")}</div>,
    ).container;
    expect(html.querySelector("a[href='javascript:alert(1)']")).toBeNull();
    const html2 = render(
      <div>{renderBBCode("[img]javascript:alert(1)[/img]")}</div>,
    ).container;
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
    const html = render(<div>{renderBBCode(src)}</div>).container;
    expect(html.querySelectorAll("img")).toHaveLength(1);
    const imdb = "a[href='https://www.imdb.com/title/tt34423706/']";
    expect(html.querySelector(imdb)).not.toBeNull();
    expect(html.querySelectorAll("blockquote")).toHaveLength(2);
    // 渲染后不该再露出任何裸标签
    expect(html.textContent).not.toMatch(/\[(?:img|url|quote|b)\]/);
  });
});
