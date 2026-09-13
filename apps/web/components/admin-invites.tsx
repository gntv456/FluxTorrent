"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P2-4：邀请管理（好学站 user/invites 口径）：全站邀请码浏览 */

interface InviteRow {
  id: number;
  inviter: string;
  inviter_id: number;
  code: string;
  status: number;
  used_by: number | null;
  used_by_name: string | null;
  expires_at: string;
}

const STATUS_LABEL: Record<number, string> = { 0: "未用", 1: "已用", 2: "过期", 3: "撤销" };

export function AdminInvites() {
  const [rows, setRows] = useState<InviteRow[]>([]);
  const [total, setTotal] = useState(0);
  const [uid, setUid] = useState("");
  const [valid, setValid] = useState("");
  const [page, setPage] = useState(1);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (uid.trim()) p.set("uid", uid.trim());
    if (valid !== "") p.set("valid", valid);
    try {
      const r = await api.get<{ rows: InviteRow[]; total: number }>(`/api/v1/admin/invites?${p}`);
      setRows(r.rows);
      setTotal(r.total);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [uid, valid, page]);

  useEffect(() => { load(); }, [load]);

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
      <section className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          发邀者 UID
          <input value={uid} onChange={(e) => { setUid(e.target.value); setPage(1); }} placeholder="留空看全部" className="min-h-[40px] w-32 rounded-[var(--r-sm)] border border-line px-2" />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          状态
          <select value={valid} onChange={(e) => { setValid(e.target.value); setPage(1); }} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
            <option value="">全部</option>
            <option value="0">未用</option>
            <option value="1">已用</option>
            <option value="2">过期</option>
            <option value="3">撤销</option>
          </select>
        </label>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">发邀者</td>
            <td className="colhead">邀请码</td>
            <td className="colhead">状态</td>
            <td className="colhead">注册用户</td>
            <td className="colhead">到期时间</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td className="num">{r.id}</td>
              <td><a href={`/admin/users/${r.inviter_id}`} className="font-bold text-link">{r.inviter}</a></td>
              <td className="font-mono">{r.code.slice(0, 8)}…</td>
              <td>{STATUS_LABEL[r.status] ?? r.status}</td>
              <td>{r.used_by ? <a href={`/admin/users/${r.used_by}`} className="text-link">{r.used_by_name ?? `#${r.used_by}`}</a> : "—"}</td>
              <td className="text-sub">{new Date(r.expires_at).toLocaleDateString()}</td>
            </tr>
          ))}
          {rows.length === 0 && <tr><td colSpan={6} className="py-6 text-center text-sub">暂无邀请码</td></tr>}
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
