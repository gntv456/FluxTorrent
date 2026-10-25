"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { UsersBatchBar, UsersTable } from "@/components/admin-users-list";
import { UsersFilterBar } from "@/components/admin-users-filters";
import { useUserFilters } from "@/components/admin-users-parts";
import { UsersPager } from "@/components/admin-users-pager";

/** 第五轮：后台用户管理（同类站 /nexusphp user/users 口径）
 * 七维筛选（ID/等级/状态/启用/下载权限/挂起/自定义字段）+ 排序 + 分页 + 批量操作。
 * 详情走独立页 /admin/users/[id]（components/admin-user-detail.tsx）；
 * 列表与批量操作拆出 admin-users-list.tsx、筛选面板拆出
 * admin-users-filters.tsx、筛选状态拆出 admin-users-parts.ts（300 门禁）。 */
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
  const { dict } = useI18n();
  const at = dict.adminUsers;
  const [sort, setSort] = useState("id");
  const [desc, setDesc] = useState(true);
  const [page, setPage] = useState(1);
  const [data, setData] = useState<UsersPage | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // P2 清偿：批量操作确认状态机（prompt 收理由 + danger 确认）
  const ask = useConfirm();
  // 第八轮 P2-9：批量操作
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [batchClass, setBatchClass] = useState("");
  /** 筛选状态（含 G4 自定义字段维）抽到 admin-users-parts.ts，主组件守 300 行 */
  const filters = useUserFilters();
  /** 自定义字段清单（G4）：筛选项用；无权/未建字段时为空数组 */
  const [fields, setFields] = useState<{ key: string; label: string }[]>([]);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    const params = filters.params({
      sort,
      desc: String(desc),
      page: String(page),
      per_page: "20",
    });
    try {
      setData(
        await api.get<UsersPage>(`/api/v1/admin/users?${params.toString()}`),
      );
      setSel(new Set());
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [filters, sort, desc, page, dict]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    api
      .get<{ key: string; label: string }[]>("/api/v1/admin/user-fields")
      .then((rs) => setFields(rs.filter((r) => r.key)))
      .catch(() => {});
  }, []);

  const totalPages = data
    ? Math.max(1, Math.ceil(data.total / data.per_page))
    : 1;

  /** P2 清偿：批量操作确认状态机。封禁类（status>0）先收理由
   *  （prompt 输入槽）再 danger 确认；其余直接 danger 确认。
   *  旧 window.prompt/confirm 退役。 */
  function askBatch(action: "status" | "class", value: number) {
    const ids = [...sel];
    if (ids.length === 0) {
      flash(at.pickFirstUser);
      return;
    }
    const needReason = action === "status" && value > 0;
    const actLabel =
      action === "status"
        ? [at.stNormalAct, at.stMuteAct, at.stBanAct][value]
        : fmt(at.classTo, { n: value });
    ask.open({
      title: actLabel,
      message: fmt(at.batchConfirm, { n: ids.length, act: actLabel }),
      tone: "danger",
      label: needReason ? at.batchReason : undefined,
      busy,
      run: (reasonText) =>
        batch(action, value, needReason ? reasonText.trim() : undefined),
    });
  }

  async function batch(
    action: "status" | "class",
    value: number,
    reason?: string,
  ) {
    const ids = [...sel];
    if (ids.length === 0) {
      flash(at.pickFirstUser);
      return;
    }
    setBusy(true);
    try {
      const r = await api.post<{ updated: number; skipped: number[] }>(
        "/api/v1/admin/users/batch",
        { action, ids, value, reason },
      );
      const skipNote =
        r.skipped.length > 0 ? fmt(at.skipNote, { n: r.skipped.length }) : "";
      flash(fmt(at.updatedMsg, { n: r.updated }) + skipNote);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
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
      {/* 筛选条件（同类站「筛选条件」面板口径） */}
      <UsersFilterBar
        classes={classes}
        fields={fields}
        values={filters.values}
        set={filters.set}
        onSearch={() => {
          setPage(1);
          load();
        }}
      />
      {/* 批量操作（第八轮 P2-9）：onClick 走 askBatch 弹确认（P2 清偿） */}
      <UsersBatchBar
        sel={sel}
        busy={busy}
        batch={askBatch}
        classes={classes}
        batchClass={batchClass}
        setBatchClass={setBatchClass}
      />
      {/* 用户列表（12 列，手机上交给外层横滚，别把每列压成一字一行） */}
      <div className="overflow-x-auto">
        <UsersTable data={data} sel={sel} setSel={setSel} sortBtn={sortBtn} />
      </div>

      {/* 分页 */}
      <UsersPager
        data={data}
        page={page}
        totalPages={totalPages}
        setPage={setPage}
      />
      <AskDialog {...ask} />
    </div>
  );
}
