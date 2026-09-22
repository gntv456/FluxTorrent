import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { api } from "@/lib/api-client";
import { useCategoryDnd } from
  "@/app/(main)/admin/forums/_parts/use-category-dnd";
import type { ForumCategory } from "@fluxtorrent/domain-types";

/** 分区拖拽排序状态机（`useCategoryDnd`）。
 *
 *  这些用例覆盖的是**无头浏览器测不到的那条路径：接口失败后的回滚**。
 *  真实浏览器里要触发它得把 api 停掉（会打断同一仓库的其它会话），
 *  所以放在单测里锁住 —— 它也是「乐观更新」唯一的风险点。
 *
 *  另外锁住一个易错点：本地顺序由**服务端顺序的字符串快照**驱动重置，
 *  不能拿 categories 数组引用做依赖（父组件每次 render 都可能给新引用，
 *  会把拖拽中的乐观顺序冲掉）。
 */

const cats = (...ids: number[]): ForumCategory[] =>
  ids.map((id) => ({
    id,
    name: `分区${id}`,
    sort: 0,
    visible: true,
    forums: 0,
  }));

/** 与生产一致的 run 包装：吞掉异常（真实实现只负责提示文案） */
function makeRun() {
  return vi.fn(async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch {
      /* 生产里在这里 setMsg；回滚由 hook 自己负责，不依赖调用方 */
    }
  });
}

const ids = (list: ForumCategory[]) => list.map((c) => c.id);

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("useCategoryDnd", () => {
  it("初始顺序 = 服务端顺序", () => {
    const { result } = renderHook(() =>
      useCategoryDnd(cats(1, 2, 3), makeRun()),
    );
    expect(ids(result.current.list)).toEqual([1, 2, 3]);
  });

  it("move() 乐观换位并提交新顺序", async () => {
    const run = makeRun();
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    const { result } = renderHook(() => useCategoryDnd(cats(1, 2, 3), run));
    act(() => result.current.move(0, 1));
    expect(ids(result.current.list)).toEqual([2, 1, 3]);
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith(
        "/api/v1/admin/forum-categories/reorder",
        { ids: [2, 1, 3] },
      ),
    );
  });

  it("边界：首行上移 / 末行下移都不发请求", () => {
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    const { result } = renderHook(() =>
      useCategoryDnd(cats(1, 2, 3), makeRun()),
    );
    act(() => result.current.move(0, -1));
    act(() => result.current.move(2, 1));
    expect(ids(result.current.list)).toEqual([1, 2, 3]);
    expect(post).not.toHaveBeenCalled();
  });

  it("接口失败 → 先乐观换位、再回滚到提交前顺序", async () => {
    let fail!: (e: unknown) => void;
    vi.spyOn(api, "post").mockReturnValue(
      new Promise((_, reject) => {
        fail = reject;
      }) as never,
    );
    const { result } = renderHook(() =>
      useCategoryDnd(cats(1, 2, 3), makeRun()),
    );
    act(() => result.current.move(0, 1));
    // 请求未决期间：乐观顺序已生效
    expect(ids(result.current.list)).toEqual([2, 1, 3]);
    await act(async () => {
      fail(new Error("boom"));
    });
    await waitFor(() => expect(ids(result.current.list)).toEqual([1, 2, 3]));
  });

  it("拖拽：startDrag(1) → overDrag(0) 实时换位，endDrag 才提交", async () => {
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    const { result } = renderHook(() =>
      useCategoryDnd(cats(1, 2, 3), makeRun()),
    );
    act(() => result.current.startDrag(1));
    act(() => result.current.overDrag(0));
    expect(ids(result.current.list)).toEqual([2, 1, 3]);
    expect(post).not.toHaveBeenCalled(); // 拖动途中不发请求
    await act(async () => result.current.endDrag());
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith(
        "/api/v1/admin/forum-categories/reorder",
        { ids: [2, 1, 3] },
      ),
    );
  });

  it("拖了但落回原位 → 不发请求", async () => {
    const post = vi.spyOn(api, "post").mockResolvedValue({} as never);
    const { result } = renderHook(() =>
      useCategoryDnd(cats(1, 2, 3), makeRun()),
    );
    act(() => result.current.startDrag(1));
    act(() => result.current.overDrag(1));
    await act(async () => result.current.endDrag());
    expect(post).not.toHaveBeenCalled();
    expect(ids(result.current.list)).toEqual([1, 2, 3]);
  });

  it("拖拽失败也回滚到拖拽前顺序", async () => {
    vi.spyOn(api, "post").mockRejectedValue(new Error("boom"));
    const { result } = renderHook(() =>
      useCategoryDnd(cats(1, 2, 3), makeRun()),
    );
    act(() => result.current.startDrag(2));
    act(() => result.current.overDrag(0));
    expect(ids(result.current.list)).toEqual([3, 1, 2]);
    await act(async () => result.current.endDrag());
    await waitFor(() => expect(ids(result.current.list)).toEqual([1, 2, 3]));
  });

  it("服务端顺序变化 → 本地顺序跟随重置（新增分区场景）", async () => {
    const run = makeRun();
    vi.spyOn(api, "post").mockResolvedValue({} as never);
    const { result, rerender } = renderHook(
      ({ c }) => useCategoryDnd(c, run),
      { initialProps: { c: cats(1, 2) } },
    );
    await act(async () => result.current.move(0, 1));
    await waitFor(() => expect(ids(result.current.list)).toEqual([2, 1]));
    // 服务端多了一个分区：本地乐观顺序必须让位给服务端真值
    rerender({ c: cats(1, 2, 3) });
    await waitFor(() => expect(ids(result.current.list)).toEqual([1, 2, 3]));
  });
});
