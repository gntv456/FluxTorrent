import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";

import { MobileHomeHeader } from "@/components/mobile-home-header";
import { homeMeSummary } from "@/app/(main)/_parts/home-me-summary";
import { zhCN } from "@/i18n/zh-CN";

/** 首页移动数据条：从取数到屏上的那一层。
 *
 *  P-464 的四格文本算在 `_parts/home-me-summary` 里，这里钉的是「组件
 *  确实把算好的字符串原样印出来」——否则格式化只改了函数、页面自己又
 *  拼一遍，纯函数用例照样全绿而屏上仍是旧数。
 */

const GIB = 1024 ** 3;
const MIB = 1024 ** 2;

type Ov = { uploaded?: number; downloaded?: number; spark_balance?: number };

/** 按标签（↑/↓/∑/★ 那一列的 dt）取同一格里显示值的文本 */
function cell(ov: Ov, labelKey: keyof typeof zhCN.mhome): string {
  const { container } = render(
    <MobileHomeHeader brand="测试站" me={homeMeSummary(ov, "zh-CN")} />,
  );
  const label = String(zhCN.mhome[labelKey]);
  // 只在**这一次** render 的容器里找：一个用例里连渲染四次，
  // screen.* 会把前面几次的同名标签一起捞出来（Found multiple elements）。
  const dts = Array.from(container.querySelectorAll("dt"));
  const dt = dts.find((d) => (d.textContent ?? "").trim() === label);
  const dd = dt?.parentElement?.querySelector("dd");
  expect(dd, `找不到标签「${label}」所在的数据格`).not.toBeNull();
  return dd?.textContent ?? "";
}

describe("MobileHomeHeader × homeMeSummary（渲染层）", () => {
  it("四格印的就是共用格式化函数的结果", () => {
    const ov: Ov = {
      uploaded: 5 * GIB,
      downloaded: 30 * MIB,
      spark_balance: 1234,
    };
    // 5 GiB / 30 MiB = 170.7 倍 ⇒ 走策划方案 §1086 的 INF 档；
    // 想看数字那档就取一个不到 100 倍的组合。
    expect(cell(ov, "up")).toBe("↑ 5.0 GB");
    expect(cell(ov, "down")).toBe("↓ 30.0 MB");
    expect(cell(ov, "ratio")).toBe("∑ INF");
    const mid: Ov = { uploaded: 150 * MIB, downloaded: 30 * MIB };
    expect(cell(mid, "up")).toBe("↑ 150 MB"); // ≥100 时不留小数（formatBytes 既有口径）
    expect(cell(mid, "ratio")).toBe("∑ 5.00");
    expect(cell(ov, "spark")).toBe("★ 1,234");
  });

  it("下载过 30 MiB 不许在屏上显示成 0", () => {
    expect(cell({ uploaded: 0, downloaded: 30 * MIB }, "down")).toBe(
      "↓ 30.0 MB",
    );
    expect(cell({ uploaded: 0, downloaded: 30 * MIB }, "down")).not.toMatch(
      /0\.0G/,
    );
  });

  it("没取到数时只剩问候语，屏上不打 NaN/undefined", () => {
    const { container } = render(
      <MobileHomeHeader brand="测试站" me={homeMeSummary(null, "zh-CN")} />,
    );
    expect(container.textContent).toContain("测试站");
    expect(container.textContent).not.toMatch(/NaN|undefined|∑/);
  });
});
