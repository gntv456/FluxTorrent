"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";

/**
 * 样图「十连」：十次**真实单抽**的客户端串行循环，每抽独立幂等键。
 * 单抽口径 / EV / 限次完全不变 —— 十连只是把同一发令枪抠十下，
 * 中途失败（限次/余额）即停，已抽的部分照常返回给调用方展示。
 */
export async function tenDraw<T>(
  path: string,
  body: Record<string, unknown> = {},
): Promise<T[]> {
  const out: T[] = [];
  const base =
    typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : String(Date.now());
  for (let i = 0; i < 10; i++) {
    out.push(
      await api.post<T>(path, {
        ...body,
        idempotency_key: `${base}-t${i}`,
      }),
    );
  }
  return out;
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
      opts.onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return { rows, open, busy, run, close: () => setOpen(false) };
}
