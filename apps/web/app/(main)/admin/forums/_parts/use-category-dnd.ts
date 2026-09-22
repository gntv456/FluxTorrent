"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ForumCategory } from "@fluxtorrent/domain-types";

/** 分区排序：↑↓ 按钮与拖拽共用同一套「乐观更新 + 失败回滚」提交路径。
 *
 *  为什么抽成 hook：面板组件本来就快 300 行（门禁上限），把排序状态机塞进去会超；
 *  而且这段逻辑有两处容易写错的地方，独立出来更看得清：
 *
 *  1. **本地顺序必须由「服务端顺序的字符串快照」驱动重置**，不能拿 `categories`
 *     数组引用做依赖 —— 父组件每次 render 都可能给新引用，会把拖拽中的
 *     乐观顺序冲掉。`join(",")` 成字符串后引用无关，只在真的变了才重置。
 *  2. **乐观更新要留回滚点**：拖拽落地时顺序已经改了，失败必须回到拖拽**前**的
 *     顺序，而不是"当前值"。
 */
export function useCategoryDnd(
  categories: ForumCategory[],
  run: (fn: () => Promise<void>, ok: string) => void,
) {
  const { dict } = useI18n();
  const serverKey = categories.map((c) => c.id).join(",");
  const [order, setOrder] = useState<number[]>(
    () => categories.map((c) => c.id),
  );
  const [dragIdx, setDragIdx] = useState<number | null>(null);
  /** 拖拽开始前的顺序，作为失败回滚点 */
  const [dragPrev, setDragPrev] = useState<number[]>([]);

  // 服务端顺序变化（提交成功后回读、或别处改动）→ 重置本地乐观顺序
  useEffect(() => {
    setOrder(serverKey ? serverKey.split(",").map(Number) : []);
  }, [serverKey]);

  // 按本地顺序重排；若与服务端集合对不上（增删分区的中间态）则直接用服务端顺序
  const byId = new Map(categories.map((c) => [c.id, c]));
  const ordered = order
    .map((id) => byId.get(id))
    .filter((c): c is ForumCategory => !!c);
  const list = ordered.length === categories.length ? ordered : categories;

  function submit(next: number[], prev: number[]) {
    setOrder(next); // 乐观：先动 UI
    run(async () => {
      try {
        await api.post("/api/v1/admin/forum-categories/reorder", {
          ids: next,
        });
      } catch (e) {
        setOrder(prev); // 失败回到提交前的顺序，再交给 run 统一提示
        throw e;
      }
    }, dict.adminForums.orderSaved);
  }

  function move(idx: number, dir: -1 | 1) {
    const j = idx + dir;
    if (j < 0 || j >= list.length) return;
    const next = [...order];
    const tmp = next[idx]!;
    next[idx] = next[j]!;
    next[j] = tmp;
    submit(next, order);
  }

  function startDrag(i: number) {
    setDragIdx(i);
    setDragPrev(order);
  }

  /** dragover 时**实时挪动**（而非交换）：被拖项跟着指针走，预览即最终结果 */
  function overDrag(i: number) {
    if (dragIdx === null || dragIdx === i) return;
    const next = [...order];
    const [moved] = next.splice(dragIdx, 1);
    next.splice(i, 0, moved!);
    setDragIdx(i);
    setOrder(next);
  }

  function endDrag() {
    const prev = dragPrev;
    setDragIdx(null);
    if (order.join(",") === prev.join(",")) return; // 没动过就不发请求
    submit(order, prev);
  }

  return { list, dragIdx, move, startDrag, overDrag, endDrag };
}
