import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { ReactNode } from "react";

import {
  classNameOf,
  useClassNames,
} from "@/lib/hooks/use-class-names";

/**
 * 等级名单 hook（2026-11-14 硬编码清偿）：i18n classList 退役后，
 * 批量发放/用户详情/后台徽章/设定页 classlevel 的等级名统一来自
 * GET /api/v1/classes（user_classes.name）。钉住三件事：
 *  1. 请求失败回落空表，不炸调用方（jsdom 无 mock 场景即生产侧未登录兜底）；
 *     放在最前——失败后 cache 仍为 null，后续用例才会真发请求；
 *  2. 成长档 + 职务档合并成一份 [id, name][]（形状与旧 classList 一致）；
 *  3. classNameOf 找不到 id 返回 null（调用方自选兜底文案）。
 *
 * 注意：hook 的名单缓存在模块级变量里，用例间不复位——顺序即语义，
 * 勿把「成功」用例挪到「失败」用例前面。
 */

function Probe({ onReady }: { onReady: (list: [number, string][]) => void }) {
  const list = useClassNames();
  return (
    <button onClick={() => onReady(list)} data-testid="probe">
      {list.length}
    </button>
  );
}

function mount(child: ReactNode) {
  render(<>{child}</>);
}

describe("useClassNames（等级名单出 i18n 归 DB）", () => {
  it("请求失败回落空表（调用方自兜底，不炸渲染）", async () => {
    const { api } = await import("@/lib/api-client");
    vi.spyOn(api, "get").mockRejectedValueOnce(new Error("network"));
    mount(<Probe onReady={() => {}} />);
    // effect 拉取是异步的：失败被 hook 吞掉，维持空表（0 = 名单未就绪）
    await vi.waitFor(() => {
      expect(api.get).toHaveBeenCalled();
    });
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.getByTestId("probe").textContent).toBe("0");
  });

  it("合并 classes + staff_classes 为一份 [id, name][]", async () => {
    const { api } = await import("@/lib/api-client");
    vi.spyOn(api, "get").mockResolvedValueOnce({
      classes: [
        { id: 1, name: "新芽" },
        { id: 2, name: "用户甲" },
      ],
      staff_classes: [{ id: 99, name: "管理员甲" }],
    });
    let got: [number, string][] | null = null;
    mount(<Probe onReady={(l) => (got = l)} />);
    // 先等真名到位（空表 → 3 条），再点探针拿全量清单验形状
    await vi.waitFor(() => {
      expect(screen.getByTestId("probe").textContent).toBe("3");
    });
    fireEvent.click(screen.getByTestId("probe"));
    expect(got).toEqual([
      [1, "新芽"],
      [2, "用户甲"],
      [99, "管理员甲"],
    ]);
  });
});

describe("classNameOf", () => {
  it("按 id 取名，找不到返回 null", () => {
    const list: [number, string][] = [
      [1, "新芽"],
      [99, "管理员甲"],
    ];
    expect(classNameOf(list, 99)).toBe("管理员甲");
    expect(classNameOf(list, 42)).toBeNull();
    expect(classNameOf(list, undefined)).toBeNull();
    expect(classNameOf(list, null)).toBeNull();
  });
});
