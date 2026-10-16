"use client";

import { INPUT_GROW } from "@/lib/ui-classes";

/**
 * 后台种子管理·登录记录子表（从 components/admin-torrents.tsx 按域拆出）：
 * LoginLogs 登录记录表格 + 一键封/解封 IP（testip 查询封禁态，
 * 同类站 login-logs 口径）。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import type { LoginRow } from "./admin-torrents-shared";
import { Pager } from "@/components/ui/pager";

// ============ 登录记录（含一键封/解封 IP，同类站 login-logs 口径） ============

export function LoginLogs() {
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  const c = dict.common;
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{
    rows: LoginRow[];
    page: number;
    total: number;
  } | null>(null);
  const [banned, setBanned] = useState<Set<string>>(new Set());
  const [msg, setMsg] = useState<string | null>(null);
  // P2 清偿：封禁理由采集（window.prompt 退役 → 输入槽）
  const ask = useConfirm();

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

  function banIp(ip: string) {
    // 理由走输入槽；空理由（确认但未填）沿用旧口径拦截
    ask.open({
      title: at.banIp,
      label: fmt(at.banPrompt, { ip }),
      run: (reason) => {
        if (!reason.trim()) return;
        void doBan(ip, reason);
      },
    });
  }
  async function doBan(ip: string, reason: string) {
    try {
      await api.post("/api/v1/admin/bans", { ip, reason });
      flash(fmt(at.bannedMsg, { ip }));
      setBanned((prev) => new Set(prev).add(ip));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    }
  }
  async function unbanIp(ip: string) {
    try {
      await api.post("/api/v1/admin/bans/by-ip/delete", { ip });
      flash(fmt(at.unbannedMsg, { ip }));
      setBanned((prev) => {
        const n = new Set(prev);
        n.delete(ip);
        return n;
      });
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
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
          placeholder={at.qLogin}
          className={INPUT_GROW}
        />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{at.thUser}</td>
            <td className="colhead">IP</td>
            <td className="colhead">{at.thGeo}</td>
            <td className="colhead">{at.thResult}</td>
            <td className="colhead">{at.thTime}</td>
            <td className="colhead">{at.thIpAction}</td>
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
                  <span className="text-sub">{at.lanUnknown}</span>
                )}
              </td>
              <td className={r.ok ? "text-mint" : "text-danger"}>
                {r.ok ? at.okMsg : at.failMsg}
              </td>
              <td className="text-xs text-sub">
                {new Date(r.created_at).toLocaleString()}
              </td>
              <td>
                {r.ip &&
                  (banned.has(r.ip) ? (
                    <button className="cmgmt-act" onClick={() => unbanIp(r.ip)}>
                      {at.unbanIp}
                    </button>
                  ) : (
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() => banIp(r.ip)}
                    >
                      {at.banIp}
                    </button>
                  ))}
              </td>
            </tr>
          ))}
          {data?.rows.length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-sub">
                {at.loginEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <Pager
        layout="compact"
        page={page}
        pages={Math.max(1, Math.ceil((data?.total ?? 0) / 20))}
        posLabel={fmt(at.pageInfo, {
          page: data?.page ?? 1,
          total: data?.total ?? 0,
        })}
        onStep={setPage}
      />
      <AskDialog {...ask} />
    </div>
  );
}
