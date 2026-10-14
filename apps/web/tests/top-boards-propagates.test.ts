import { describe, it, expect, vi } from "vitest";

/**
 * P-309：`getTopBoards` 必须把接口故障传出去。
 *
 * 页面 `app/(main)/top/page.tsx` 按 W-27 自己 try/catch，用来区分
 * 「接口挂 → ErrorPanel」与「真的全空 → EmptyState」。但数据层原先
 * `catch { return 六榜空 }`，异常在这一层就被吞掉：页面的 catch 是死代码，
 * 后端故障会被演成六榜「暂无数据」——正是页面注释声称已经修掉的形状。
 *
 * 两条一起跑：故障必须抛出（修前会 resolve 成空对象 ⇒ 红），
 * 正常返回必须原样透出（防「一律抛」的假修）。
 */

vi.mock("@/lib/api-client", () => ({
  api: { get: vi.fn() },
}));

import { api } from "@/lib/api-client";
import { getTopBoards } from "@/lib/data-forum";

const BOARDS = [
  "bonus",
  "uploaded",
  "downloaded",
  "seedtime",
  "hourly",
  "torrents",
] as const;

describe("getTopBoards 的故障口径（P-309）", () => {
  it("接口挂时必须抛出，而不是 resolve 成六榜全空", async () => {
    vi.mocked(api.get).mockRejectedValueOnce(
      new Error("top/boards 500 内部错误"),
    );
    await expect(getTopBoards()).rejects.toThrow("500");
  });

  it("正控：正常返回原样透出，六榜键一个不少", async () => {
    const payload = {
      bonus: [{ username: "a", val: 1 }],
      uploaded: [],
      downloaded: [],
      seedtime: [],
      hourly: [],
      torrents: [],
    };
    vi.mocked(api.get).mockResolvedValueOnce(
      payload as unknown as Awaited<ReturnType<typeof getTopBoards>>,
    );
    const got = await getTopBoards();
    expect(Object.keys(got).sort()).toEqual([...BOARDS].sort());
    expect(got.bonus).toEqual([{ username: "a", val: 1 }]);
  });
});
