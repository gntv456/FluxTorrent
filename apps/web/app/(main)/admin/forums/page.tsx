"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { AdminPageShell } from "@/components/admin-page-shell";
import { ForumCategoryPanel } from "./_parts/forum-category-panel";
import { ForumBoardTable } from "./_parts/forum-board-table";
import { ForumBoardDrawer } from "./_parts/forum-board-drawer";
import {
  NO_CATEGORY,
  catOf,
  type ForumAdminData,
  type ForumAdminForum,
} from "./_parts/forum-structure-types";

const MSG_CLS = "mb-3 rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink";

/** 论坛结构（独立整页 /admin/forums）。
 *
 *  替代原先「后台某个 tool tab 里的一个表格 + 两个堆在下面的表单」：
 *  左栏分区树（改名前移可见性）+ 右栏该分区的版块表，分区与版块同屏闭环。
 *  后台导航条目已指向本页，`/admin?tool=forums` 旧深链也会跳过来。 */
export default function AdminForumsPage() {
  const { dict } = useI18n();
  const f = dict.adminForums;
  const [data, setData] = useState<ForumAdminData | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const [editing, setEditing] = useState<ForumAdminForum | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setData(await api.get<ForumAdminData>("/api/v1/admin/forums"));
      setLoadFailed(false);
    } catch {
      setLoadFailed(true);
    }
  }, []);

  // 导航条目的拉取交给 AdminPageShell（与 /admin/settings 共用）
  useEffect(() => {
    load();
  }, [load]);

  // 首次拿到数据后默认选中第一个分区
  useEffect(() => {
    if (selected !== null) return;
    const first = data?.categories?.[0];
    if (first) setSelected(first.id);
  }, [data, selected]);

  /** 统一的操作包装：失败提示交底（不再一律显示「Load 失败或无权限」） */
  async function run(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      setMsg(ok);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : f.actionFailed);
    } finally {
      setBusy(false);
    }
  }

  /** 左侧导航切换已交给 AdminPageShell（整页跳转，URL 为唯一真相） */

  const cats = data?.categories ?? [];
  const all = data?.forums ?? [];
  const shown =
    selected === null ? [] : all.filter((f) => catOf(f) === selected);
  const curName =
    selected === NO_CATEGORY
      ? f.noCategory
      : (cats.find((c) => c.id === selected)?.name ?? "—");

  return (
    <AdminPageShell active="forums">
      {msg && <p className={MSG_CLS}>{msg}</p>}

      {loadFailed ? (
        <div className="baozi-panel p-6 text-center text-sm text-sub">
          <p className="mb-2">{f.loadFailed}</p>
          <button
            type="button"
            className="text-sky"
            onClick={() => {
              setMsg(null);
              load();
            }}
          >
            {f.reload}
          </button>
        </div>
      ) : data === null ? (
        <p className="py-8 text-center text-sm text-sub">
          {dict.common.loading}
        </p>
      ) : (
        <div className="baozi-panel p-4">
          <div className="mb-3 flex items-center gap-3">
            {/* 唯一 h1：顶栏标题已降为视觉 p（admin-shell），页面标题自持 */}
            <h1 className="font-display text-base font-bold text-ink">
              {f.title}
            </h1>
            <span className="text-xs text-sub">{f.hint}</span>
          </div>

          <div className="flex items-stretch">
            <ForumCategoryPanel
              categories={cats}
              forums={all}
              selected={selected ?? NO_CATEGORY}
              onSelect={setSelected}
              busy={busy}
              run={run}
            />
            <ForumBoardTable
              forums={shown}
              mods={data.mods ?? []}
              catName={curName}
              busy={busy}
              run={run}
              onEdit={(f) => {
                setEditing(f);
                setDrawerOpen(true);
              }}
              onNew={() => {
                setEditing(null);
                setDrawerOpen(true);
              }}
            />
          </div>
        </div>
      )}

      <ForumBoardDrawer
        open={drawerOpen}
        forum={editing}
        categories={cats}
        classes={data?.classes ?? []}
        defaultCatId={
          selected === null || selected === NO_CATEGORY ? "" : selected
        }
        busy={busy}
        run={run}
        onClose={() => setDrawerOpen(false)}
      />
    </AdminPageShell>
  );
}
