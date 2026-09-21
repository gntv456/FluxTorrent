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
});
