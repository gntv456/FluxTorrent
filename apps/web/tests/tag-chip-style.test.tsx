import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { tagPaint, tagChipStyle } from "@/lib/tag-style";
import { TagChip } from "@/components/forum-bits";

/**
 * 标签芯片配色（2026-11-02 站长报：标签字典列表里 官种/合集/完结/官方
 * 的预览是空白方块，连字都看不见）。
 *
 * 根因是存库形状「bg_color='' + color='#ffffff'」——站型包导入只写名字，
 * 底色与字色都落列默认，而 `#ffffff` 这个默认预设了「背后一定有底色」。
 * 本用例钉的是渲染侧那条规矩：**字色跟着底色走**，没底色就整套交回主题。
 * 数据侧由迁移 0453 抹平（列默认改成空串 + 清掉矛盾值）。
 *
 * 负控：把 tagChipStyle 退回 `color: tag.color`（信存库值），
 * 「隐形形状」那条必须红。
 */

const INVISIBLE = { bg_color: "", color: "#ffffff", font_size: "12px" };
const PAINTED = { bg_color: "#006400", color: "#ffffff", font_size: "12px" };
// TagChip 要 name（域类型），纯样式用例不带也不给
const CHIP_INVISIBLE = { ...INVISIBLE, name: "官种" } as never;
const CHIP_PAINTED = { ...PAINTED, name: "中字" } as never;

describe("tagPaint：有底色才算配了色", () => {
  it("空底色 = 没配色（返回 null，交回主题）", () => {
    expect(tagPaint(INVISIBLE)).toBeNull();
    expect(tagPaint({ bg_color: "   ", color: "#fff" })).toBeNull();
    expect(tagPaint(null)).toBeNull();
  });

  it("有底色：原样用存库配色", () => {
    expect(tagPaint(PAINTED)).toEqual({
      background: "#006400",
      color: "#ffffff",
    });
  });

  it("有底色但字色漏填 → 回落白字（深底白字是芯片惯例）", () => {
    expect(tagPaint({ bg_color: "#ff0000", color: "" })?.color).toBe("#ffffff");
  });

  it("底色可以是令牌串（表单默认 var(--sky)），照原样交给浏览器解析", () => {
    expect(tagPaint({ bg_color: "var(--sky)", color: "#0c3a4a" })?.background)
      .toBe("var(--sky)");
  });
});

describe("tagChipStyle：不许产出看不见的芯片", () => {
  it("隐形形状 → 字色必须是主题墨色，不能是存库的白", () => {
    const s = tagChipStyle(INVISIBLE);
    expect(s.color).toBe("var(--ink)");
    expect(s.color).not.toBe("#ffffff");
    expect(s.background).toBe("var(--surface-sunken)");
  });

  it("后台预览用透明底做缺省，但字色照样交回主题", () => {
    const s = tagChipStyle(INVISIBLE, "transparent");
    expect(s.background).toBe("transparent");
    expect(s.color).toBe("var(--ink)");
  });

  it("配了色的标签不受影响", () => {
    const s = tagChipStyle(PAINTED);
    expect(s.background).toBe("#006400");
    expect(s.color).toBe("#ffffff");
  });

  it("字号/内边距/圆角照存库值透传（论坛侧自己覆盖字号，不在这把尺里）", () => {
    const s = tagChipStyle({
      bg_color: "#006400",
      color: "#fff",
      font_size: "14px",
      padding: "2px 8px",
      border_radius: "9999px",
    });
    expect(s.fontSize).toBe("14px");
    expect(s.padding).toBe("2px 8px");
    expect(s.borderRadius).toBe("9999px");
  });

  it("正控：库里那 4 条真实形状都判得出来（官种/合集/完结/官方）", () => {
    for (const r of [
      { bg_color: "", color: "#ffffff" },
      { bg_color: "", color: "#ffffff" },
      { bg_color: "", color: "#ffffff" },
      { bg_color: "", color: "#ffffff" },
    ]) {
      expect(tagChipStyle(r).color).toBe("var(--ink)");
    }
    // 而有底色的白字标签不许被误伤（否则这条判据只是「一律不白」）
    expect(tagChipStyle({ bg_color: "#8F77B5", color: "#ffffff" }).color)
      .toBe("#ffffff");
  });
});

/**
 * 接线判据：纯函数绿不等于渲染层绿——这里挂真组件读 DOM 上的 style 属性。
 * jsdom 读 `el.style.color` 对 `var(…)` 可能回空串，所以比 attribute 原文。
 */
describe("TagChip（论坛标签芯片）渲染层", () => {
  it("隐形形状的标签挂出来必须带主题墨色", () => {
    const { container } = render(<TagChip tag={CHIP_INVISIBLE} />);
    const el = container.querySelector("span") as HTMLElement;
    const style = el.getAttribute("style") || "";
    expect(style).toContain("var(--ink)");
    expect(style).not.toMatch(/color:\s*#ffffff/i);
    expect(el.textContent).toBe("官种");
  });

  it("配了色的标签照存库值渲染（不是被兜底吃掉）", () => {
    const { container } = render(
      <TagChip tag={CHIP_PAINTED} />,
    );
    const el = container.querySelector("span") as HTMLElement;
    const style = el.getAttribute("style") || "";
    expect(style).toContain("rgb(0, 100, 0)");
    expect(el.textContent).toBe("中字");
  });
});
