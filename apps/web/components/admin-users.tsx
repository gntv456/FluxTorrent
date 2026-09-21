"use client";

import { BTN_SM_GHOST } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  UserDetailPanel,
  type AdjustState,
  type AdminUserDetail,
} from "@/components/admin-users-detail";
import { UsersBatchBar, UsersTable } from "@/components/admin-users-list";
import { UsersFilterBar } from "@/components/admin-users-filters";
import { UsersPager } from "@/components/admin-users-pager";

/** 第五轮：后台用户管理（好学站 /nexusphp user/users 口径）
 * 五维筛选（ID/等级/状态/启用/下载权限/挂起）+ 排序 + 分页 + 详情 + 数值调整 + 开关。
 * 用户详情面板拆出 admin-users-detail.tsx、列表与批量操作拆出
 * admin-users-list.tsx、筛选面板拆出 admin-users-filters.tsx（300 门禁）。 */
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

// 分页按钮
const PAGE_BTN = BTN_SM_GHOST;

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
    const actLabel =
      action === "status"
        ? ["恢复正常", "禁言", "封禁"][value]
        : `等级改为 ${value}`;
    if (!window.confirm(`确认对 ${ids.length} 个用户执行「${actLabel}」？`))
      return;
    setBusy(true);
    try {
      const r = await api.post<{ updated: number; skipped: number[] }>(
        "/api/v1/admin/users/batch",
        { action, ids, value, reason },
      );
      const skipNote =
        r.skipped.length > 0 ? `，跳过（等级不足）${r.skipped.length} 个` : "";
      flash(`已更新 ${r.updated} 个用户${skipNote}`);
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
      <UsersFilterBar
        classes={classes}
        values={{ q, fId, fClass, fStatus, fEnabled, fDownload, fSuspended }}
        set={(k, v) => {
          if (k === "q") setQ(v);
          else if (k === "fId") setFId(v);
          else if (k === "fClass") setFClass(v);
          else if (k === "fStatus") setFStatus(v);
          else if (k === "fEnabled") setFEnabled(v);
          else if (k === "fDownload") setFDownload(v);
          else setFSuspended(v);
        }}
        onSearch={() => {
          setPage(1);
          load();
        }}
      />

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
      <UsersPager
        data={data}
        page={page}
        totalPages={totalPages}
        setPage={setPage}
      />

      {/* 用户详情（好学站用户详情页口径：字段全景 + 管理动作） */}
      {detail && (
        <UserDetailPanel
          detail={detail}
          onClose={() => setDetail(null)}
          onAdjust={() =>
            setAdjust({
              up: "0",
              down: "0",
              spark: "0",
              invite: "0",
              note: "",
            })
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
