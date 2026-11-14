import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";

import { ScratchCoat } from "@/components/game/scratch-coat";

/**
 * 刮格错峰（手感批 2026-11-13）：批量揭开（一次 +≥2 格，即「一键全开」）
 * 时挂 data-stagger 交给 CSS 逐格延迟；单格手刮不挂。
 * 组件不直接渲染延迟数字——节奏在 arcade-fx.css，这里断言 class 开关。
 */

const base = {
  outcome: { mult: 2, icon: "✨", rarity: 2, net: 100 },
  hitIndex: 0,
  onReveal: () => {},
  decoyLabel: "谢谢参与",
  disabled: false,
};

describe("ScratchCoat stagger", () => {
  it("单格揭开：不挂 data-stagger", () => {
    const { container } = render(
      <ScratchCoat {...base} revealed={[2]} />,
    );
    const grid = container.querySelector(".sw-coat-grid");
    expect(grid).toBeTruthy();
    expect(grid?.hasAttribute("data-stagger")).toBe(false);
  });

  it("批量揭开（6 格全开）：挂 data-stagger", () => {
    const { container } = render(
      <ScratchCoat {...base} revealed={[0, 1, 2, 3, 4, 5]} />,
    );
    const grid = container.querySelector(".sw-coat-grid");
    expect(grid?.hasAttribute("data-stagger")).toBe(true);
  });

  it("全开 800ms 后错峰标记自动摘除（计时器真实等待）", async () => {
    const { container } = render(
      <ScratchCoat {...base} revealed={[0, 1, 2, 3, 4, 5]} />,
    );
    const grid = container.querySelector(".sw-coat-grid");
    expect(grid?.hasAttribute("data-stagger")).toBe(true);
    await new Promise((r) => setTimeout(r, 900));
    expect(grid?.hasAttribute("data-stagger")).toBe(false);
  });
});
