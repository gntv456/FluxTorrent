import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";

import { WheelDisc } from "@/components/game/wheel-disc";
import { WheelPoolPanel } from "@/components/game/wheel-pool";
import type { WheelPrize } from "@/lib/wheel-prize";

/** 18 档与迁移 0510 同形：谢谢 519 + 17 档，权重合计 1000。 */
const PRIZES: WheelPrize[] = [
  ["谢谢参与", 519, 0, 1],
  ["0.5x 返本", 55, 0.5, 2],
  ["1.1x 魔力", 85, 1.1, 2],
  ["1.2x 魔力", 72, 1.2, 2],
  ["1.5x 魔力", 107, 1.5, 3],
  ["1.8x 魔力", 40, 1.8, 3],
  ["2x 魔力", 38, 2, 3],
  ["2.5x 魔力", 26, 2.5, 3],
  ["3x 魔力", 20, 3, 3],
  ["4x 魔力", 12, 4, 3],
  ["5x 魔力", 8, 5, 4],
  ["6x 魔力", 6, 6, 4],
  ["8x 魔力", 4, 8, 4],
  ["10x 大奖", 3, 10, 4],
  ["12x 大奖", 2, 12, 5],
  ["20x 大奖", 1, 20, 5],
  ["30x 传奇", 1, 30, 5],
  ["50x 传奇", 1, 50, 5],
].map(([label, weight, payout, rarity]) => ({
  label: label as string,
  weight_permille: weight as number,
  payout: payout as number,
  value: (payout as number) * 100,
  rarity: rarity as number,
  kind: "magic",
}));

const LABELS = {
  picked: "高价值奖池",
  gridTitle: "转盘奖池 · 概率公示",
  note: "扇区与后端奖池同源",
  empty: "奖池尚未配置",
  metaItem: "限定道具",
  metaItemQty: "×{n} 件",
  metaMiss: "未中奖",
  metaMult: "×{n} 倍率",
};

describe("大转盘盘面（18 档密盘）", () => {
  it("按档位画满 18 个扇区，并写出倍率读数", () => {
    const { container } = render(
      <WheelDisc prizes={PRIZES} ticket={100} hitIndex={null} />,
    );
    expect(container.querySelectorAll("path")).toHaveLength(18);
    expect(container.textContent).toContain("×50");
    expect(container.textContent).toContain("×1.5");
    expect(container.textContent).toContain("🍀");
  });

  it("中奖扇区被单独描金（不靠 CSS 类，重转时不会残留）", () => {
    const { container } = render(
      <WheelDisc prizes={PRIZES} ticket={100} hitIndex={17} />,
    );
    const paths = container.querySelectorAll("path");
    expect(paths[17].getAttribute("stroke-width")).toBe("3.5");
    expect(paths[0].getAttribute("stroke-width")).toBe("1.6");
  });
});

describe("奖池概率页", () => {
  it("18 档全部上墙，概率按千分权重换算", () => {
    const { container } = render(
      <WheelPoolPanel prizes={PRIZES} ticket={100} labels={LABELS} />,
    );
    const items = container.querySelectorAll(".wp-pool-item");
    expect(items).toHaveLength(18);
    expect(container.textContent).toContain("51.9%");
    expect(container.textContent).toContain("×50 倍率");
  });

  it("顶部精选条只放价值最高的 4 档", () => {
    const { container } = render(
      <WheelPoolPanel prizes={PRIZES} ticket={100} labels={LABELS} />,
    );
    expect(
      container.querySelectorAll(".sw-prize-item"),
    ).toHaveLength(4);
  });
});
