"use client";

import { BTN_SM_GHOST, INPUT_GROW } from "@/lib/ui-classes";

/**
 * 后台种子管理·通用记录查询（从 components/admin-torrents.tsx 按域拆出）：
 * RecordQuery 按用户名搜索 + 任意列配置的分页表格（火花/购买记录共用）。
 */

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { RecordRow } from "./admin-torrents-shared";

// ============ 通用记录查询（火花/购买，既有） ============

export function RecordQuery({
  title,
  endpoint,
  columns,
}: {
  title: string;
  endpoint: string;
  columns: [string, string][];
}) {
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  const c = dict.common;
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{ rows: RecordRow[]; page: number } | null>(
    null,
  );

  useEffect(() => {
    const params = new URLSearchParams();
    if (q.trim()) params.set("q", q.trim());
    params.set("page", String(page));
    api
      .get<{ rows: RecordRow[]; page: number } | null>(
        `${endpoint}?${params.toString()}`,
      )
      .then(setData)
      .catch(() => setData(null));
  }, [q, page, endpoint]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-2">
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={fmt(at.qRecord, { title })}
          className={INPUT_GROW}
        />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            {columns.map(([k, label]) => (
              <td key={k} className="colhead">
                {label}
              </td>
            ))}
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((r) => (
            <tr key={r.id}>
              {columns.map(([k]) => {
                const v = r[k];
                return (
                  <td key={k} className="text-xs">
                    {k === "created_at" && typeof v === "string"
                      ? new Date(v).toLocaleString()
                      : k === "ok"
                        ? v
                          ? at.okMsg
                          : at.failMsg
                        : String(v ?? "—")}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-end gap-2 text-sm text-sub">
        <button
          disabled={page <= 1}
          onClick={() => setPage(page - 1)}
          className={BTN_SM_GHOST}
        >
          {c.prevPage}
        </button>
        <span>{fmt(at.pageNum, { n: data?.page ?? 1 })}</span>
        <button
          disabled={!data || data.rows.length < 20}
          onClick={() => setPage(page + 1)}
          className={BTN_SM_GHOST}
        >
          {c.nextPage}
        </button>
      </div>
    </div>
  );
}
