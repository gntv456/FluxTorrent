"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
/** 第五轮：后台种子管理 + 拒绝原因字典 + 种子操作记录 + 记录查询（好学站口径） */
interface AdminTorrentRow {
  id: number;
  name: string;
  owner_id: number | null;
  owner_name: string | null;
  category_id: number;
  size: number;
  seeders: number;
  leechers: number;
  approval_status: number;
  deny_reason: string | null;
  deny_note: string | null;
  sticky: boolean;
  created_at: string;
}

interface DenyReason {
  id: number;
  sort: number;
  reason: string;
  enabled: boolean;
}

interface TorrentOpRow {
  id: number;
  torrent_id: number;
  torrent_name: string | null;
  operator_name: string | null;
  action: string;
  detail: Record<string, unknown> | null;
  created_at: string;
}

interface RecordRow {
  id: number;
  username: string;
  [k: string]: unknown;
}

const APPROVAL = ["待审", "通过", "拒绝"];

function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

type SubTab = "torrents" | "deny" | "ops" | "spark" | "buys" | "logins";

export function AdminTorrents() {
  const [sub, setSub] = useState<SubTab>("torrents");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2" role="tablist">
        {([
          ["torrents", "种子管理"],
          ["deny", "拒绝原因"],
          ["ops", "种子操作记录"],
          ["spark", "火花记录"],
          ["buys", "种子购买"],
          ["logins", "登录记录"],
        ] as [SubTab, string][]).map(([k, label]) => (
          <button
            key={k}
            role="tab"
            aria-selected={sub === k}
            onClick={() => setSub(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${sub === k ? "bg-sky text-white" : "border border-line bg-white text-sub"}`}
          >
            {label}
          </button>
        ))}
      </div>
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
      {sub === "torrents" && <TorrentList flash={flash} />}
      {sub === "deny" && <DenyReasons flash={flash} />}
      {sub === "ops" && <OpLogs />}
      {sub === "spark" && <RecordQuery title="火花记录" endpoint="/api/v1/admin/spark-logs" columns={[["username", "用户"], ["amount", "数额"], ["kind", "类型"], ["balance_after", "余额"], ["created_at", "时间"]]} />}
      {sub === "buys" && <RecordQuery title="种子购买记录" endpoint="/api/v1/admin/torrent-buys" columns={[["username", "用户"], ["kind", "类型"], ["ref_id", "种子ID"], ["amount", "火花"], ["created_at", "时间"]]} />}
      {sub === "logins" && <RecordQuery title="登录记录" endpoint="/api/v1/admin/login-logs" columns={[["username", "用户"], ["ip", "IP"], ["ok", "结果"], ["created_at", "时间"]]} />}
    </div>
  );
}

function TorrentList({ flash }: { flash: (m: string) => void }) {
  const [q, setQ] = useState("");
  const [status, setStatus] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{ rows: AdminTorrentRow[]; page: number; per_page: number } | null>(null);

  const load = useCallback(async () => {
    const params = new URLSearchParams();
    if (q.trim()) params.set("q", q.trim());
    if (status) params.set("status", status);
    params.set("page", String(page));
    try {
      setData(await api.get(`/api/v1/admin/torrents?${params.toString()}`));
    } catch {
      setData(null);
    }
  }, [q, status, page]);

  useEffect(() => {
    load();
  }, [load]);

  const decide = async (id: number, approve: boolean) => {
    let deny_reason_id: number | undefined;
    let reason = "";
    if (!approve) {
      const reasons: DenyReason[] = await api.get("/api/v1/admin/deny-reasons");
      const choice = prompt(`拒绝原因（输入编号，可留空后手填理由）：\n${reasons.map((r) => `${r.id}. ${r.reason}`).join("\n")}\n或直接输入自定义理由文字`);
      if (!choice) return;
      const asNum = Number(choice);
      if (asNum > 0 && reasons.some((r) => r.id === asNum)) deny_reason_id = asNum;
      else reason = choice;
    }
    try {
      await api.post("/api/v1/admin/reviews/decide", { torrent_id: id, approve, reason, deny_reason_id });
      flash(approve ? `已通过种子 #${id}` : `已拒绝种子 #${id}`);
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2">
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="种子名" className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2" />
        <select value={status} onChange={(e) => setStatus(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-white px-2">
          <option value="">全部状态</option>
          <option value="1">待审</option>
          <option value="2">通过</option>
          <option value="3">拒绝</option>
          <option value="4">死种</option>
        </select>
        <button onClick={() => { setPage(1); load(); }} className="min-h-[40px] rounded-full bg-sky px-4 text-xs font-bold text-white">
          搜索
        </button>
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">名称</td>
            <td className="colhead">发布者</td>
            <td className="colhead">大小</td>
            <td className="colhead">做种/下载</td>
            <td className="colhead">状态</td>
            <td className="colhead">拒绝原因</td>
            <td className="colhead">操作</td>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((t) => (
            <tr key={t.id}>
              <td>{t.id}</td>
              <td className="max-w-[240px] truncate">
                <a className="font-bold text-link" href={`/torrents?id=${t.id}`}>{t.name}</a>
              </td>
              <td className="text-xs">{t.owner_name ?? "—"}</td>
              <td className="text-xs">{fmtBytes(t.size)}</td>
              <td>{t.seeders} / {t.leechers}</td>
              <td>{APPROVAL[t.approval_status] ?? t.approval_status}</td>
              <td className="text-xs">{t.deny_reason ?? (t.deny_note || "—")}</td>
              <td>
                {t.approval_status === 0 && (
                  <>
                    <button className="cmgmt-act" onClick={() => decide(t.id, true)}>通过</button>
                    <button className="cmgmt-act cmgmt-act--danger" onClick={() => decide(t.id, false)}>拒绝</button>
                  </>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-between text-sm text-sub">
        <span />
        <div className="flex gap-2">
          <button disabled={page <= 1} onClick={() => setPage(page - 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">上一页</button>
          <span>第 {data?.page ?? 1} 页</span>
          <button disabled={!data || data.rows.length < 20} onClick={() => setPage(page + 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">下一页</button>
        </div>
      </div>
    </div>
  );
}

function DenyReasons({
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

function OpLogs() {
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

function RecordQuery({
  title,
  endpoint,
  columns,
}: {
  title: string;
  endpoint: string;
  columns: [string, string][];
}) {
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{ rows: RecordRow[]; page: number } | null>(null);

  useEffect(() => {
    const params = new URLSearchParams();
    if (q.trim()) params.set("q", q.trim());
    params.set("page", String(page));
    api.get<{ rows: RecordRow[]; page: number } | null>(`${endpoint}?${params.toString()}`).then(setData).catch(() => setData(null));
  }, [q, page, endpoint]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-2">
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder={`${title}：按用户名搜索`} className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2" />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            {columns.map(([k, label]) => (
              <td key={k} className="colhead">{label}</td>
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
                          ? "成功"
                          : "失败"
                        : String(v ?? "—")}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-end gap-2 text-sm text-sub">
        <button disabled={page <= 1} onClick={() => setPage(page - 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">上一页</button>
        <span>第 {data?.page ?? 1} 页</span>
        <button disabled={!data || data.rows.length < 20} onClick={() => setPage(page + 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">下一页</button>
      </div>
    </div>
  );
}
