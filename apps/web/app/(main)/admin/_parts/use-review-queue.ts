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
  const [reasons, setReasons] = useState<DenyReason[]>([]);
  const [reasonId, setReasonId] = useState<number | null>(null);
  const pageSize = 50;

  const reload = useCallback(async () => {
    const d = await api.get<QueueEnvelope>(
      `/api/v1/admin/reviews?limit=${pageSize}&offset=${offset}`,
    );
    setItems(d.items ?? []);
    setTotal(d.total ?? 0);
  }, [offset]);

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
  };
}
