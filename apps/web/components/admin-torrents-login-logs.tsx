"use client";

/**
 * 后台种子管理·登录记录子表（从 components/admin-torrents.tsx 按域拆出）：
 * LoginLogs 登录记录表格 + 一键封/解封 IP（testip 查询封禁态，
 * 好学站 login-logs 口径）。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import type { LoginRow } from "./admin-torrents-shared";

// ============ 登录记录（含一键封/解封 IP，好学站 login-logs 口径） ============

export function LoginLogs() {
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{
    rows: LoginRow[];
    page: number;
    total: number;
  } | null>(null);
  const [banned, setBanned] = useState<Set<string>>(new Set());
  const [msg, setMsg] = useState<string | null>(null);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (q.trim()) p.set("q", q.trim());
    api
      .get<{ rows: LoginRow[]; page: number; total: number } | null>(
        `/api/v1/admin/login-logs?${p.toString()}`,
      )
      .then(setData)
      .catch(() => setData(null));
  }, [q, page]);
  useEffect(() => {
    load();
  }, [load]);

  // 页内出现过的 IP 逐一查询封禁态（testip 复用，轻量）
  useEffect(() => {
    const ips = [
      ...new Set((data?.rows ?? []).map((r) => r.ip).filter(Boolean)),
    ];
    ips.forEach(async (ip) => {
      try {
        const r = await api.get<{ banned: boolean }>(
          `/api/v1/admin/testip?ip=${encodeURIComponent(ip)}`,
        );
        if (r.banned) setBanned((prev) => new Set(prev).add(ip));
      } catch {
        /* 忽略 */
      }
    });
  }, [data]);

  async function banIp(ip: string) {
    const reason = prompt(`封禁 ${ip} 的理由：`);
    if (!reason) return;
    try {
      await api.post("/api/v1/admin/bans", { ip, reason });
      flash(`已封禁 ${ip}`);
      setBanned((prev) => new Set(prev).add(ip));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    }
  }
  async function unbanIp(ip: string) {
    try {
      await api.post("/api/v1/admin/bans/by-ip/delete", { ip });
      flash(`已解封 ${ip}`);
      setBanned((prev) => {
        const n = new Set(prev);
        n.delete(ip);
        return n;
      });
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <div className="flex gap-2">
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="按用户名 / IP 搜索"
          className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2"
        />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">用户</td>
            <td className="colhead">IP</td>
            <td className="colhead">国家/城市</td>
            <td className="colhead">结果</td>
            <td className="colhead">时间</td>
            <td className="colhead">IP 操作</td>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((r) => (
            <tr key={r.id}>
              <td>{r.username}</td>
              <td className="font-mono text-xs">{r.ip}</td>
              <td className="text-xs">
                {r.country_name || r.country ? (
                  `${r.country_name ?? r.country}${r.city ? ` · ${r.city}` : ""}`
                ) : (
                  <span className="text-sub">内网/未知</span>
                )}
              </td>
              <td className={r.ok ? "text-mint" : "text-danger"}>
                {r.ok ? "成功" : "失败"}
              </td>
              <td className="text-xs text-sub">
                {new Date(r.created_at).toLocaleString()}
              </td>
              <td>
                {r.ip &&
                  (banned.has(r.ip) ? (
                    <button className="cmgmt-act" onClick={() => unbanIp(r.ip)}>
                      解封 IP
                    </button>
                  ) : (
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() => banIp(r.ip)}
                    >
                      封禁 IP
                    </button>
                  ))}
              </td>
            </tr>
          ))}
          {data?.rows.length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-sub">
                暂无登录记录
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <div className="flex items-center justify-end gap-2 text-sm text-sub">
        <button
          disabled={page <= 1}
          onClick={() => setPage(page - 1)}
          className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
        >
          上一页
        </button>
        <span>
          第 {data?.page ?? 1} 页 / 共 {data?.total ?? 0} 条
        </span>
        <button
          disabled={!data || data.rows.length < 20}
          onClick={() => setPage(page + 1)}
          className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
        >
          下一页
        </button>
      </div>
    </div>
  );
}
