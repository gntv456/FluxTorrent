"use client";

/**
 * 后台种子管理·记录类子表（从 components/admin-torrents.tsx 按域拆出）：
 * DenyReasons 拒绝原因字典、OpLogs 种子操作记录。
 * LoginLogs 在 ./admin-torrents-login-logs.tsx；
 * RecordQuery 在 ./admin-torrents-record-query.tsx。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import type { DenyReason, TorrentOpRow } from "./admin-torrents-shared";

// ============ 拒绝原因（既有） ============

export function DenyReasons({
  flash,
}: {
  flash: (m: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [rows, setRows] = useState<DenyReason[]>([]);
  const [edit, setEdit] = useState<{ id: number | null; reason: string; sort: number }>({ id: null, reason: "", sort: 0 });

  const load = useCallback(async () => {
    try {
      setRows(await api.get("/api/v1/admin/deny-reasons"));
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const save = async () => {
    if (!edit.reason.trim()) return;
    setBusy(true);
    try {
      if (edit.id === null) await api.post("/api/v1/admin/deny-reasons", { reason: edit.reason, sort: edit.sort });
      else await api.put(`/api/v1/admin/deny-reasons/${edit.id}`, { reason: edit.reason, sort: edit.sort });
      flash("已保存");
      setEdit({ id: null, reason: "", sort: 0 });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold text-ink">{edit.id === null ? "新增拒绝原因" : `编辑 #${edit.id}`}</h2>
        <label>
          原因文本
          <input value={edit.reason} onChange={(e) => setEdit({ ...edit, reason: e.target.value })} />
        </label>
        <label>
          排序
          <input type="number" value={edit.sort} onChange={(e) => setEdit({ ...edit, sort: Number(e.target.value) })} />
        </label>
        <div className="flex gap-2">
          <button className="baozi-button" disabled={busy || !edit.reason.trim()} onClick={save}>
            保存
          </button>
          {edit.id !== null && (
            <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setEdit({ id: null, reason: "", sort: 0 })}>
              取消
            </button>
          )}
        </div>
      </section>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">排序</td>
            <td className="colhead">原因</td>
            <td className="colhead">启用</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>{r.sort}</td>
              <td>{r.reason}</td>
              <td>{r.enabled ? "是" : "否"}</td>
              <td className="text-right">
                <button className="cmgmt-act" onClick={() => setEdit({ id: r.id, reason: r.reason, sort: r.sort })}>编辑</button>
                <button
                  className="cmgmt-act"
                  onClick={async () => {
                    try {
                      await api.put(`/api/v1/admin/deny-reasons/${r.id}`, { enabled: !r.enabled });
                      flash(r.enabled ? "已停用" : "已启用");
                      load();
                    } catch {
                      flash("操作失败");
                    }
                  }}
                >
                  {r.enabled ? "停用" : "启用"}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/deny-reasons/${r.id}`);
                      flash("已删除");
                      load();
                    } catch {
                      flash("删除失败");
                    }
                  }}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ============ 种子操作记录（既有） ============

export function OpLogs() {
  const [tid, setTid] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{ rows: TorrentOpRow[]; total: number; page: number } | null>(null);

  useEffect(() => {
    const params = new URLSearchParams();
    if (tid.trim()) params.set("torrent_id", tid.trim());
    params.set("page", String(page));
    api.get<{ rows: TorrentOpRow[]; total: number; page: number } | null>(`/api/v1/admin/torrent-ops?${params.toString()}`).then(setData).catch(() => setData(null));
  }, [tid, page]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-2">
        <input value={tid} onChange={(e) => setTid(e.target.value)} placeholder="按种子 ID 过滤（留空看全部）" className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2" />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">种子</td>
            <td className="colhead">操作人</td>
            <td className="colhead">动作</td>
            <td className="colhead">详情</td>
            <td className="colhead">时间</td>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td className="max-w-[200px] truncate text-xs">
                #{r.torrent_id} {r.torrent_name ?? ""}
              </td>
              <td>{r.operator_name ?? "—"}</td>
              <td>{r.action}</td>
              <td className="max-w-[220px] truncate text-xs">{r.detail ? JSON.stringify(r.detail) : "—"}</td>
              <td className="text-xs">{new Date(r.created_at).toLocaleString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-end gap-2 text-sm text-sub">
        <button disabled={page <= 1} onClick={() => setPage(page - 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">上一页</button>
        <span>
          第 {data?.page ?? 1} 页 / 共 {data?.total ?? 0} 条
        </span>
        <button disabled={!data || data.rows.length < 20} onClick={() => setPage(page + 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">下一页</button>
      </div>
    </div>
  );
}

