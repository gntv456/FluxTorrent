"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { UsersBatchBar, UsersTable } from "@/components/admin-users-list";
import { UsersFilterBar } from "@/components/admin-users-filters";
import {
  EMPTY_FILTERS,
  buildUserFilterParams,
  useUserFilters,
} from "@/components/admin-users-parts";
import type { UserFilterValues } from "@/components/admin-users-parts";
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
  /** 已生效的筛选快照：文本输入 debounce 后推进，选择项变更/搜索/翻页/排序立即推进；
   *  表单草稿本身不参与查询（避免每次键入都发请求）。 */
  const [applied, setApplied] = useState(filters.values);
  /** 自定义字段清单（G4）：筛选项用；无权/未建字段时为空数组 */
  const [fields, setFields] = useState<{ key: string; label: string }[]>([]);
  /** 文本输入防抖计时器 + 草稿最新值（同一事件内连改两次也不丢） */
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const draft = useRef(filters.values);
  /** 请求序号：慢响应回来时若序号已过期就丢弃，避免旧结果覆盖新结果 */
  const seq = useRef(0);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  /** 取消待触发的 debounce（点搜索 / 清空 / 收起 / 卸载时用） */
  const cancelPending = useCallback(() => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
  }, []);

  /** 推进生效快照：取消待触发请求、回到第 1 页（与旧「搜索」语义一致） */
  const commit = useCallback(
    (next: UserFilterValues) => {
      cancelPending();
      setApplied(next);
      setPage(1);
    },
    [cancelPending],
  );

  /** 筛选项变更：文本位 400ms 防抖，选择位（下拉/开关）立即生效 */
  const change = useCallback(
    <K extends keyof UserFilterValues>(k: K, v: UserFilterValues[K]) => {
      const next = { ...draft.current, [k]: v };
      draft.current = next;
      filters.set(k, v);
      if (k === "q" || k === "fId" || k === "fVal") {
        cancelPending();
        timer.current = setTimeout(() => {
          timer.current = null;
          commit(next);
        }, 400);
      } else {
        commit(next);
      }
    },
    [filters, commit, cancelPending],
  );

  /** 重置：草稿与已生效快照一起清空，取消待触发 debounce，回第 1 页并立即生效。
   *  目标恒为同一个 EMPTY_FILTERS 引用——本来就没条件时 setState 是 no-op，不发请求。 */
  const resetFilters = useCallback(() => {
    draft.current = EMPTY_FILTERS;
    filters.reset();
    commit(EMPTY_FILTERS);
  }, [filters, commit]);

  /** 有可清的条件（草稿或已生效）才让重置按钮可点：留 400ms 窗口内也能取消待触发查询 */
  const hasFilters = Boolean(
    filters.values.q.trim() ||
      filters.values.fId.trim() ||
      filters.values.fClass ||
      filters.values.fStatus ||
      filters.values.fEnabled ||
      filters.values.fDownload ||
      filters.values.fSuspended ||
      filters.values.fField ||
      applied.q.trim() ||
      applied.fId.trim() ||
      applied.fClass ||
      applied.fStatus ||
      applied.fEnabled ||
      applied.fDownload ||
      applied.fSuspended ||
      applied.fField,
  );

  // 卸载/切页清理定时器：别让组件没了还发请求
  useEffect(() => cancelPending, [cancelPending]);

  const load = useCallback(async () => {
    const params = buildUserFilterParams(applied, {
      sort,
      desc: String(desc),
      page: String(page),
      per_page: "20",
    });
    const my = ++seq.current;
    try {
      const rs = await api.get<UsersPage>(
        `/api/v1/admin/users?${params.toString()}`,
      );
      if (my !== seq.current) return; // 过期响应：已被更新的查询取代
      setData(rs);
      setSel(new Set());
    } catch (e) {
      if (my !== seq.current) return;
      flash(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [applied, sort, desc, page, dict]);

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
        applied={applied}
        set={change}
        onSearch={() => commit(draft.current)}
        onReset={resetFilters}
        canReset={hasFilters}
        onCollapse={cancelPending}
        total={data?.total}
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
