"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  UserDetailPanel,
  type AdjustState,
  type AdminUserDetail,
} from "@/components/admin-users-detail";
import { UsersBatchBar, UsersTable } from "@/components/admin-users-list";

/** 第五轮：后台用户管理（好学站 /nexusphp user/users 口径）
 * 五维筛选（ID/等级/状态/启用/下载权限/挂起）+ 排序 + 分页 + 详情 + 数值调整 + 开关。
 * 用户详情面板拆出 admin-users-detail.tsx、列表与批量操作拆出
 * admin-users-list.tsx（300 门禁）。 */
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

interface UsersPage {
  rows: AdminUserRow[];
  total: number;
  page: number;
  per_page: number;
}

export function AdminUsers({ classes }: { classes: [number, string][] }) {
  const { dict, currency } = useI18n();
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
  const [adjust, setAdjust] = useState<AdjustState | null>(null);
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
      setData(
        await api.get<UsersPage>(`/api/v1/admin/users?${params.toString()}`),
      );
      setSel(new Set());
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [
    q,
    fId,
    fClass,
    fStatus,
    fEnabled,
    fDownload,
    fSuspended,
    sort,
    desc,
    page,
    dict,
  ]);

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

  const toggleFlag = async (
    userId: number,
    flag: "download_enabled" | "suspended",
    value: boolean,
  ) => {
    setBusy(true);
    try {
      await api.put("/api/v1/admin/users/flags", {
        user_id: userId,
        [flag]: value,
      });
      flash(
        flag === "suspended"
          ? value
            ? "已挂起"
            : "已解除挂起"
          : value
            ? "已恢复下载权限"
            : "已禁用下载权限",
      );
      await openDetail(userId);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  };

  const totalPages = data
    ? Math.max(1, Math.ceil(data.total / data.per_page))
    : 1;

  async function batch(action: "status" | "class", value: number) {
    const ids = [...sel];
    if (ids.length === 0) {
      flash("请先勾选用户");
      return;
    }
    const reason =
      action === "status" && value > 0
        ? (window.prompt("批量操作理由（可选）") ?? undefined)
        : undefined;
    if (
      !window.confirm(
        `确认对 ${ids.length} 个用户执行「${action === "status" ? ["恢复正常", "禁言", "封禁"][value] : `等级改为 ${value}`}」？`,
      )
    )
      return;
    setBusy(true);
    try {
      const r = await api.post<{ updated: number; skipped: number[] }>(
        "/api/v1/admin/users/batch",
        { action, ids, value, reason },
      );
      flash(
        `已更新 ${r.updated} 个用户${r.skipped.length > 0 ? `，跳过（等级不足）${r.skipped.length} 个` : ""}`,
      );
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
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

      {/* 筛选条件（好学站「筛选条件」面板口径） */}
      <section className="baozi-panel grid grid-cols-2 gap-3 p-4 md:grid-cols-4">
        <label className="flex flex-col gap-1 text-xs">
          ID
          <input
            value={fId}
            onChange={(e) => setFId(e.target.value)}
            placeholder="UID"
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2"
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          等级
          <select
            value={fClass}
            onChange={(e) => setFClass(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
          >
            <option value="">所有</option>
            {classes.map(([id, label]) => (
              <option key={id} value={id}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          状态
          <select
            value={fStatus}
            onChange={(e) => setFStatus(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
          >
            <option value="">所有</option>
            <option value="1">正常</option>
            <option value="2">禁言</option>
            <option value="3">封禁</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          启用
          <select
            value={fEnabled}
            onChange={(e) => setFEnabled(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
          >
            <option value="">所有</option>
            <option value="yes">是</option>
            <option value="no">否</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          下载权限
          <select
            value={fDownload}
            onChange={(e) => setFDownload(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
          >
            <option value="">所有</option>
            <option value="yes">有</option>
            <option value="no">无</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          挂起
          <select
            value={fSuspended}
            onChange={(e) => setFSuspended(e.target.value)}
            className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
          >
            <option value="">所有</option>
            <option value="yes">是</option>
            <option value="no">否</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs md:col-span-2">
          搜索
          <div className="flex gap-2">
            <input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="用户名 / 邮箱"
              className="min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2"
            />
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
      <UsersBatchBar
        sel={sel}
        busy={busy}
        batch={batch}
        classes={classes}
        batchClass={batchClass}
        setBatchClass={setBatchClass}
      />

      {/* 用户列表 */}
      <UsersTable data={data} sel={sel} setSel={setSel} sortBtn={sortBtn} />

      {/* 分页 */}
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {data?.total ?? 0} 条</span>
        <div className="flex items-center gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
          >
            上一页
          </button>
          <span>
            {data?.page ?? 1} / {totalPages}
          </span>
          <button
            disabled={page >= totalPages}
            onClick={() => setPage(page + 1)}
            className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
          >
            下一页
          </button>
        </div>
      </div>

      {/* 用户详情（好学站用户详情页口径：字段全景 + 管理动作） */}
      {detail && (
        <UserDetailPanel
          detail={detail}
          onClose={() => setDetail(null)}
          onAdjust={() =>
            setAdjust({ up: "0", down: "0", spark: "0", invite: "0", note: "" })
          }
          submitAdjust={submitAdjust}
          toggleFlag={toggleFlag}
          adjust={adjust}
          setAdjust={setAdjust}
          busy={busy}
          currency={currency}
        />
      )}
    </div>
  );
}
