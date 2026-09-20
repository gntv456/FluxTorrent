"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 保种认领面板（从 admin-p2-tools.tsx 按域拆出，300 门禁）：
 *  认领记录浏览（状态/关键词筛选 + 分页）与移出保种区。 */

interface ClaimRow {
  torrent_id: number;
  torrent_name: string | null;
  seeders: number;
  claimed_by: string | null;
  claimed_at: string | null;
  seed_time_delta: number;
  uploaded_delta: number;
  exited_at: string | null;
  exit_reason: string | null;
}

function fmtDeltaSec(sec: number): string {
  if (sec <= 0) return "0 小时";
  return `${(sec / 3600).toFixed(1)} 小时`;
}

function fmtBytes(n: number): string {
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

export function Claims({ flash }: { flash: (m: string) => void }) {
  const [rows, setRows] = useState<ClaimRow[]>([]);
  const [state, setState] = useState("all");
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);

  const load = useCallback(async () => {
    const params = new URLSearchParams();
    params.set("state", state);
    if (q.trim()) params.set("q", q.trim());
    params.set("page", String(page));
    try {
      const r = await api.get<{ rows: ClaimRow[]; total: number }>(`/api/v1/admin/claims?${params.toString()}`);
      setRows(r.rows);
      setTotal(r.total);
    } catch {
      setRows([]);
    }
  }, [state, q, page]);
  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2">
        <select value={state} onChange={(e) => { setState(e.target.value); setPage(1); }} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
          <option value="all">全部</option>
          <option value="active">认领中</option>
          <option value="unclaimed">待认领</option>
          <option value="exited">已移出</option>
        </select>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="种子名 / 认领人" className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2" />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">种子</td>
            <td className="colhead">做种数</td>
            <td className="colhead">认领人</td>
            <td className="colhead">认领时间</td>
            <td className="colhead">认领以来做种</td>
            <td className="colhead">认领以来上传</td>
            <td className="colhead">状态</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.torrent_id}>
              <td className="max-w-[200px] truncate">
                <a className="text-link" href={`/torrents?id=${r.torrent_id}`}>{r.torrent_name ?? `#${r.torrent_id}`}</a>
              </td>
              <td>{r.seeders}</td>
              <td>{r.claimed_by ?? "—"}</td>
              <td className="text-xs">{r.claimed_at ? new Date(r.claimed_at).toLocaleString() : "—"}</td>
              <td>{r.claimed_by ? fmtDeltaSec(r.seed_time_delta) : "—"}</td>
              <td>{r.claimed_by ? fmtBytes(r.uploaded_delta) : "—"}</td>
              <td className="text-xs">{r.exited_at ? `已移出（${r.exit_reason ?? "manual"}）` : r.claimed_by ? "认领中" : "待认领"}</td>
              <td className="text-right">
                {!r.exited_at && (
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={async () => {
                      try {
                        await api.post("/api/v1/admin/claims/release", { torrent_id: r.torrent_id });
                        flash(`已移出 #${r.torrent_id}`);
                        load();
                      } catch (e) {
                        flash(e instanceof ApiError ? e.message : "操作失败");
                      }
                    }}
                  >
                    移出保种区
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {total} 条</span>
        <div className="flex gap-2">
          <button disabled={page <= 1} onClick={() => setPage(page - 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">上一页</button>
          <span>第 {page} 页</span>
          <button disabled={rows.length < 20} onClick={() => setPage(page + 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">下一页</button>
        </div>
      </div>
    </div>
  );
}
