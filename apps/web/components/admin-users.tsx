"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 第五轮：后台用户管理（好学站 /nexusphp user/users 口径）
 * 五维筛选（ID/等级/状态/启用/下载权限/挂起）+ 排序 + 分页 + 详情 + 数值调整 + 开关 */
interface AdminUserRow {
  id: number;
  username: string;
  email: string;
  class_id: number;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  status: number;
  download_enabled: boolean;
  suspended: boolean;
  created_at: string;
  last_seen_at: string | null;
}

interface AdminUserDetail extends AdminUserRow {
  passkey: string;
  title: string | null;
  spark_balance: number;
  parked: boolean;
  donor: boolean;
  totp_enabled: boolean;
  invited_by: number | null;
  inviter_name: string | null;
  seeding: number;
  leeching: number;
  uploads: number;
  invites_unused: number;
}

interface UsersPage {
  rows: AdminUserRow[];
  total: number;
  page: number;
  per_page: number;
}

const STATUS_LABELS = ["正常", "禁言", "封禁"];

function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

export function AdminUsers({ classes }: { classes: [number, string][] }) {
  const { dict } = useI18n();
  const [q, setQ] = useState("");
  const [fId, setFId] = useState("");
  const [fClass, setFClass] = useState("");
  const [fStatus, setFStatus] = useState("");
  const [fEnabled, setFEnabled] = useState("");
  const [fDownload, setFDownload] = useState("");
  const [fSuspended, setFSuspended] = useState("");
  const [sort, setSort] = useState("id");
  const [desc, setDesc] = useState(true);
  const [page, setPage] = useState(1);
  const [data, setData] = useState<UsersPage | null>(null);
  const [detail, setDetail] = useState<AdminUserDetail | null>(null);
  const [adjust, setAdjust] = useState<{ up: string; down: string; spark: string; invite: string; note: string } | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 第八轮 P2-9：批量操作
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [batchClass, setBatchClass] = useState("");

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    const params = new URLSearchParams();
    if (q.trim()) params.set("q", q.trim());
    if (fId.trim()) params.set("id", fId.trim());
    if (fClass) params.set("class_id", fClass);
    if (fStatus) params.set("status", fStatus);
    if (fEnabled) params.set("enabled", fEnabled);
    if (fDownload) params.set("download", fDownload);
    if (fSuspended) params.set("suspended", fSuspended);
    params.set("sort", sort);
    params.set("desc", String(desc));
    params.set("page", String(page));
    params.set("per_page", "20");
    try {
      setData(await api.get<UsersPage>(`/api/v1/admin/users?${params.toString()}`));
      setSel(new Set());
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [q, fId, fClass, fStatus, fEnabled, fDownload, fSuspended, sort, desc, page, dict]);

  useEffect(() => {
    load();
  }, [load]);

  const openDetail = async (id: number) => {
    try {
      setDetail(await api.get<AdminUserDetail>(`/api/v1/admin/users/${id}`));
      setAdjust(null);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  };

  const submitAdjust = async () => {
    if (!detail || !adjust) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/users/adjust", {
        user_id: detail.id,
        uploaded_delta: Number(adjust.up) || 0,
        downloaded_delta: Number(adjust.down) || 0,
        spark_delta: Number(adjust.spark) || 0,
        invite_grant: Number(adjust.invite) || 0,
        note: adjust.note || undefined,
      });
      flash(`已调整用户 #${detail.id}`);
      setAdjust(null);
      await openDetail(detail.id);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  };

  const toggleFlag = async (userId: number, flag: "download_enabled" | "suspended", value: boolean) => {
    setBusy(true);
    try {
      await api.put("/api/v1/admin/users/flags", { user_id: userId, [flag]: value });
      flash(flag === "suspended" ? (value ? "已挂起" : "已解除挂起") : value ? "已恢复下载权限" : "已禁用下载权限");
      await openDetail(userId);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  };

  const totalPages = data ? Math.max(1, Math.ceil(data.total / data.per_page)) : 1;

  async function batch(action: "status" | "class", value: number) {
    const ids = [...sel];
    if (ids.length === 0) { flash("请先勾选用户"); return; }
    const reason = action === "status" && value > 0 ? (window.prompt("批量操作理由（可选）") ?? undefined) : undefined;
    if (!window.confirm(`确认对 ${ids.length} 个用户执行「${action === "status" ? ["恢复正常", "禁言", "封禁"][value] : `等级改为 ${value}`}」？`)) return;
    setBusy(true);
    try {
      const r = await api.post<{ updated: number; skipped: number[] }>("/api/v1/admin/users/batch", { action, ids, value, reason });
      flash(`已更新 ${r.updated} 个用户${r.skipped.length > 0 ? `，跳过（等级不足）${r.skipped.length} 个` : ""}`);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const sortBtn = (key: string, label: string) => (
    <button
      onClick={() => {
        if (sort === key) setDesc(!desc);
        else {
          setSort(key);
          setDesc(true);
        }
      }}
      className="font-bold"
    >
      {label}
      {sort === key ? (desc ? " ↓" : " ↑") : ""}
    </button>
  );

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      {/* 筛选条件（好学站「筛选条件」面板口径） */}
      <section className="baozi-panel grid grid-cols-2 gap-3 p-4 md:grid-cols-4">
        <label className="flex flex-col gap-1 text-xs">
          ID
          <input value={fId} onChange={(e) => setFId(e.target.value)} placeholder="UID" className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          等级
          <select value={fClass} onChange={(e) => setFClass(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
            <option value="">所有</option>
            {classes.map(([id, label]) => (
              <option key={id} value={id}>{label}</option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          状态
          <select value={fStatus} onChange={(e) => setFStatus(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
            <option value="">所有</option>
            <option value="1">正常</option>
            <option value="2">禁言</option>
            <option value="3">封禁</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          启用
          <select value={fEnabled} onChange={(e) => setFEnabled(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
            <option value="">所有</option>
            <option value="yes">是</option>
            <option value="no">否</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          下载权限
          <select value={fDownload} onChange={(e) => setFDownload(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
            <option value="">所有</option>
            <option value="yes">有</option>
            <option value="no">无</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          挂起
          <select value={fSuspended} onChange={(e) => setFSuspended(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
            <option value="">所有</option>
            <option value="yes">是</option>
            <option value="no">否</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs md:col-span-2">
          搜索
          <div className="flex gap-2">
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="用户名 / 邮箱" className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2" />
            <button
              onClick={() => {
                setPage(1);
                load();
              }}
              className="min-h-[40px] rounded-full bg-sky px-4 text-xs font-bold text-white"
            >
              搜索
            </button>
          </div>
        </label>
      </section>

      {/* 批量操作（第八轮 P2-9） */}
      <section className="baozi-panel flex flex-wrap items-end gap-2 p-3">
        <p className="w-full text-xs font-bold text-sub">批量操作（已选 {sel.size} 个；只能操作等级低于自己的用户）</p>
        <button disabled={busy || sel.size === 0} onClick={() => batch("status", 0)} className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white disabled:opacity-50">批量恢复正常</button>
        <button disabled={busy || sel.size === 0} onClick={() => batch("status", 1)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50">批量禁言</button>
        <button disabled={busy || sel.size === 0} onClick={() => batch("status", 2)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger disabled:opacity-50">批量封禁</button>
        <label className="flex flex-col gap-1 text-xs">
          批量改等级
          <select value={batchClass} onChange={(e) => setBatchClass(e.target.value)} className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
            <option value="">（选择等级）</option>
            {classes.filter(([id]) => id > 0 && id < 99).map(([id, label]) => (
              <option key={id} value={id}>{id} {label}</option>
            ))}
          </select>
        </label>
        <button disabled={busy || sel.size === 0 || !batchClass} onClick={() => batch("class", Number(batchClass))} className="min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white disabled:opacity-50">执行</button>
      </section>

      {/* 用户列表 */}
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead w-10">
              <input
                type="checkbox"
                checked={(data?.rows.length ?? 0) > 0 && data!.rows.every((u) => sel.has(u.id))}
                onChange={(e) => setSel(new Set(e.target.checked ? data!.rows.map((u) => u.id) : []))}
              />
            </td>
            <td className="colhead">{sortBtn("id", "Id")}</td>
            <td className="colhead">用户名</td>
            <td className="colhead">邮箱</td>
            <td className="colhead">{sortBtn("class", "等级")}</td>
            <td className="colhead">{sortBtn("uploaded", "上传量")}</td>
            <td className="colhead">{sortBtn("downloaded", "下载量")}</td>
            <td className="colhead">状态</td>
            <td className="colhead">下载权限</td>
            <td className="colhead">挂起</td>
            <td className="colhead">{sortBtn("created", "添加时间")}</td>
            <td className="colhead">操作</td>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((u) => (
            <tr key={u.id}>
              <td>
                <input
                  type="checkbox"
                  checked={sel.has(u.id)}
                  onChange={(e) => setSel((prev) => { const n = new Set(prev); if (e.target.checked) n.add(u.id); else n.delete(u.id); return n; })}
                />
              </td>
              <td>{u.id}</td>
              <td>
                <a className="font-bold text-link" href={`/admin/users/${u.id}`}>
                  {u.username}
                </a>
                {u.status > 0 && <span className="ml-1 rounded-full bg-coral/20 px-2 py-0.5 text-[10px] text-danger">{STATUS_LABELS[u.status] ?? u.status}</span>}
              </td>
              <td className="text-xs text-sub">{u.email}</td>
              <td>{u.class_name ?? `LV${u.class_id}`}</td>
              <td>{fmtBytes(u.uploaded)}</td>
              <td>{fmtBytes(u.downloaded)}</td>
              <td>{u.status >= 2 ? "封禁" : u.status === 1 ? "禁言" : "正常"}</td>
              <td>{u.download_enabled ? "yes" : "no"}</td>
              <td>{u.suspended ? "yes" : "no"}</td>
              <td className="text-xs">{new Date(u.created_at).toLocaleDateString()}</td>
              <td>
                <a className="cmgmt-act" href={`/admin/users/${u.id}`}>详情</a>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {/* 分页 */}
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {data?.total ?? 0} 条</span>
        <div className="flex items-center gap-2">
          <button disabled={page <= 1} onClick={() => setPage(page - 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">
            上一页
          </button>
          <span>
            {data?.page ?? 1} / {totalPages}
          </span>
          <button disabled={page >= totalPages} onClick={() => setPage(page + 1)} className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40">
            下一页
          </button>
        </div>
      </div>

      {/* 用户详情（好学站用户详情页口径：字段全景 + 管理动作） */}
      {detail && (
        <section className="baozi-panel flex flex-col gap-3 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h2 className="text-base font-bold text-ink">
              用户详情 · {detail.username}（#{detail.id}）
            </h2>
            <button className="min-h-[36px] rounded-full border border-line px-3 text-xs" onClick={() => setDetail(null)}>
              关闭
            </button>
          </div>
          <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-3">
            <div><dt className="text-sub">邮箱</dt><dd>{detail.email}</dd></div>
            <div><dt className="text-sub">Passkey</dt><dd className="font-mono text-xs">{detail.passkey.slice(0, 10)}…</dd></div>
            <div><dt className="text-sub">等级</dt><dd>{detail.class_name ?? `LV${detail.class_id}`}</dd></div>
            <div><dt className="text-sub">上传量</dt><dd>{fmtBytes(detail.uploaded)}</dd></div>
            <div><dt className="text-sub">下载量</dt><dd>{fmtBytes(detail.downloaded)}</dd></div>
            <div><dt className="text-sub">火花</dt><dd>{detail.spark_balance}</dd></div>
            <div><dt className="text-sub">做种中</dt><dd>{detail.seeding}</dd></div>
            <div><dt className="text-sub">下载中</dt><dd>{detail.leeching}</dd></div>
            <div><dt className="text-sub">发布种子</dt><dd>{detail.uploads}</dd></div>
            <div><dt className="text-sub">未用邀请</dt><dd>{detail.invites_unused}</dd></div>
            <div><dt className="text-sub">邀请人</dt><dd>{detail.inviter_name ?? "—"}</dd></div>
            <div><dt className="text-sub">两步验证</dt><dd>{detail.totp_enabled ? "已开启" : "未开启"}</dd></div>
            <div><dt className="text-sub">添加时间</dt><dd>{new Date(detail.created_at).toLocaleString()}</dd></div>
            <div><dt className="text-sub">最后访问</dt><dd>{detail.last_seen_at ? new Date(detail.last_seen_at).toLocaleString() : "—"}</dd></div>
          </dl>

          <div className="flex flex-wrap gap-2">
            <button className="baozi-button" onClick={() => setAdjust({ up: "0", down: "0", spark: "0", invite: "0", note: "" })}>
              修改上传量等
            </button>
            <button
              className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${detail.download_enabled ? "border border-line text-danger" : "bg-mint text-white"}`}
              disabled={busy}
              onClick={() => toggleFlag(detail.id, "download_enabled", !detail.download_enabled)}
            >
              {detail.download_enabled ? "禁用下载权限" : "恢复下载权限"}
            </button>
            <button
              className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${detail.suspended ? "bg-mint text-white" : "border border-line text-danger"}`}
              disabled={busy}
              onClick={() => toggleFlag(detail.id, "suspended", !detail.suspended)}
            >
              {detail.suspended ? "解除挂起" : "挂起账号"}
            </button>
          </div>

          {/* 数值调整表单（delta 语义） */}
          {adjust && (
            <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
              <p className="mb-2 text-xs text-sub">正数增加、负数减少（下限 0）；邀请正数增发、负数回收。</p>
              <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
                <label className="flex flex-col gap-1 text-xs">
                  上传量增量（字节）
                  <input type="number" value={adjust.up} onChange={(e) => setAdjust({ ...adjust, up: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" />
                </label>
                <label className="flex flex-col gap-1 text-xs">
                  下载量增量（字节）
                  <input type="number" value={adjust.down} onChange={(e) => setAdjust({ ...adjust, down: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" />
                </label>
                <label className="flex flex-col gap-1 text-xs">
                  火花增量
                  <input type="number" value={adjust.spark} onChange={(e) => setAdjust({ ...adjust, spark: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" />
                </label>
                <label className="flex flex-col gap-1 text-xs">
                  邀请增发/回收
                  <input type="number" value={adjust.invite} onChange={(e) => setAdjust({ ...adjust, invite: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" />
                </label>
              </div>
              <label className="mt-2 flex flex-col gap-1 text-xs">
                备注（入审计）
                <input value={adjust.note} onChange={(e) => setAdjust({ ...adjust, note: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" />
              </label>
              <div className="mt-2 flex gap-2">
                <button className="baozi-button" disabled={busy} onClick={submitAdjust}>
                  提交调整
                </button>
                <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setAdjust(null)}>
                  取消
                </button>
              </div>
            </div>
          )}
        </section>
      )}
    </div>
  );
}
