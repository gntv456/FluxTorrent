"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import type { DenyReason } from "@/components/admin-torrents-shared";
import type { PendingTorrent } from "./admin-shared";

/**
 * 审核队列状态（0288 从 admin/page.tsx 拆出，300 行门禁）。
 *
 * 服务端旧口径是 `ORDER BY id LIMIT 200` 且无分页 —— 实测积压 207 条时最新那条
 * 根本不在返回里，新提交种子的种对审核员永久不可见。这里带 offset/total 分页，
 * 默认按提交时间最老优先（先到的先审），并顺带拉拒因字典给审核台用。
 */
export interface ReviewQueueState {
  items: PendingTorrent[];
  total: number;
  offset: number;
  pageSize: number;
  reasons: DenyReason[];
  reasonId: number | null;
  setReasonId: (id: number | null) => void;
  setPage: (offset: number) => void;
  reload: () => Promise<void>;
  /** 0306：审核态视图——0=待审（默认）· 4=暂缓（可取回、可恢复）。
   *  旧口径只拉 status=0，暂缓完的种从台上消失，审核员再也找不回它。 */
  status: number;
  setStatus: (s: number) => void;
  /** 0306：暂缓恢复（4→0）——暂缓不是终态，证据补齐后回待审队列。
   *  内部已切回待审视图并重拉；成功回 true（调用方只补反馈文案）。 */
  resume: (torrentId: number) => Promise<boolean>;
}

interface QueueEnvelope {
  total: number;
  limit: number;
  offset: number;
  items: PendingTorrent[];
}

export function useReviewQueue(): ReviewQueueState {
  const [items, setItems] = useState<PendingTorrent[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [status, setStatus] = useState(0);
  const [reasons, setReasons] = useState<DenyReason[]>([]);
  const [reasonId, setReasonId] = useState<number | null>(null);
  const pageSize = 50;

  // 取数抽成带参函数：resume 要在 setStatus 后立刻按新态重拉，
  // 不能依赖会随 state 重建的 reload 闭包（否则拿到旧 status 的列表）。
  const fetchQueue = useCallback(async (off: number, st: number) => {
    const d = await api.get<QueueEnvelope>(
      `/api/v1/admin/reviews?limit=${pageSize}` +
        `&offset=${off}&status=${st}`,
    );
    setItems(d.items ?? []);
    setTotal(d.total ?? 0);
  }, []);

  const reload = useCallback(async () => {
    await fetchQueue(offset, status);
  }, [fetchQueue, offset, status]);

  useEffect(() => {
    reload().catch(() => {});
  }, [reload]);

  // 拒因字典只在审核台用得到，但仍一次性拉取（7 条，几毫秒）
  useEffect(() => {
    api
      .get<DenyReason[]>("/api/v1/admin/deny-reasons")
      .then(setReasons)
      .catch(() => {});
  }, []);

  const resume = useCallback(
    async (torrentId: number) => {
      await api.post("/api/v1/admin/reviews/resume", {
        torrent_id: torrentId,
        reason: "",
      });
      setOffset(0);
      setStatus(0);
      await fetchQueue(0, 0);
      return true;
    },
    [fetchQueue],
  );

  return {
    items,
    total,
    offset,
    pageSize,
    reasons,
    reasonId,
    setReasonId,
    setPage: setOffset,
    reload,
    status,
    setStatus,
    resume,
  };
}
