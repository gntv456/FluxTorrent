"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { clientUuid } from "@/lib/client-uuid";

/**
 * 样图「十连」：十次**真实单抽**的客户端串行循环，每抽独立幂等键。
 * 单抽口径 / EV / 限次完全不变 —— 十连只是把同一发令枪抠十下，
 * 中途失败（限次/余额）即停，已抽的部分照常返回给调用方展示：
 * 循环内逐发 push，失败时把**已结算的 results + 错误**一起交回，
 * 调用方据此展示部分结果（钱已扣的部分不能静默吞掉）。
 */
export async function tenDraw<T>(
  path: string,
  body: Record<string, unknown> = {},
): Promise<T[]> {
  const out: T[] = [];
  const base = clientUuid();
  for (let i = 0; i < 10; i++) {
    try {
      out.push(
        await api.post<T>(path, {
          ...body,
          idempotency_key: `${base}-t${i}`,
        }),
      );
    } catch (e) {
      // 已有部分结果：带上错误抛「部分完成」信封，调用方可展示已抽部分
      if (out.length > 0) {
        throw new TenDrawPartial(out, e);
      }
      throw e;
    }
  }
  return out;
}

/** 第 k 发失败但前 k-1 发已结算：results 是已成功的抽，cause 是原始错误 */
export class TenDrawPartial<T> extends Error {
  constructor(
    public readonly results: T[],
    public readonly cause0: unknown,
  ) {
    super(
      cause0 instanceof Error ? cause0.message : String(cause0),
    );
    this.name = "TenDrawPartial";
  }
}

/** 十连的页面侧状态机：行数据 / 浮层开关 / 忙态。错误文案由页面合成。 */
export function useTenDraw<T>(opts: {
  path: string;
  body?: Record<string, unknown>;
  onError: (msg: string) => void;
  onDone?: () => void;
  netOf: (r: T) => number;
  labelOf: (r: T) => string;
}) {
  const [rows, setRows] = useState<{ label: string; net: number }[]>([]);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  async function run() {
    if (busy) return;
    setBusy(true);
    try {
      const rs = await tenDraw<T>(opts.path, opts.body ?? {});
      setRows(rs.map((r) => ({ label: opts.labelOf(r), net: opts.netOf(r) })));
      setOpen(true);
      opts.onDone?.();
    } catch (e) {
      // 部分完成：先展示已结算的几发，再报错（钱已扣的结果不能吞）。
      // instanceof 不带泛型（TS 不允许 instantiation expression）
      if (e instanceof TenDrawPartial) {
        const part = e as TenDrawPartial<T>;
        if (part.results.length > 0) {
          setRows(
            part.results.map((r) => ({
              label: opts.labelOf(r),
              net: opts.netOf(r),
            })),
          );
          setOpen(true);
          opts.onDone?.();
        }
        opts.onError(e.message);
      } else {
        opts.onError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setBusy(false);
    }
  }

  return { rows, open, busy, run, close: () => setOpen(false) };
}
