"use client";

import { useCallback, useState } from "react";
import { api } from "@/lib/api-client";

import type { ForumAdminData } from "./staff-tools-forums";

/** 论坛版块管理面板（从 components/staff-tools-forums.tsx 按域拆出）：
 *  分区/节点管理（0115）——列表 + 新建 + 删除
 *  （删分区不删版块，版块回落未分组）。 */

// 新建版块/分区表单的输入框
const INPUT_CLS =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-3 text-sm outline-none focus:border-sky";
// 主操作蓝色按钮 / 次级描边按钮
const BTN_SKY =
  "min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold " +
  "text-white disabled:opacity-50";
const BTN_LINE =
  "min-h-[40px] rounded-full border border-line px-4 text-sm text-sub";
// 分区标签胶囊
const CAT_TAG =
  "inline-flex items-center gap-1 rounded-full border " +
  "border-line px-2 py-1 text-xs";

export function CategoryManager({
  forumData,
  setForumData,
  busy,
  guard,
}: {
  forumData: ForumAdminData | null;
  setForumData: React.Dispatch<React.SetStateAction<ForumAdminData | null>>;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => void;
}) {
  const [fNewCat, setFNewCat] = useState("");

  return (
    <div className="mt-4 border-t border-line pt-4">
      <h3 className="mb-2 text-sm font-bold">分区管理</h3>
      <div className="mb-2 flex flex-wrap gap-2">
        {(forumData?.categories ?? []).map((c) => (
          <span key={c.id} className={CAT_TAG}>
            {c.name}
            <button
              className="font-bold text-danger"
              title="删除分区（版块回落未分组）"
              onClick={() =>
                guard(async () => {
                  await api.del(`/api/v1/admin/forum-categories/${c.id}`);
                  setForumData(await api.get("/api/v1/admin/forums"));
                }, "已删除分区")
              }
            >
              ×
            </button>
          </span>
        ))}
        {(forumData?.categories ?? []).length === 0 && (
          <span className="text-xs text-sub">暂无分区</span>
        )}
      </div>
      <div className="flex items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">新分区名</span>
          <input
            value={fNewCat}
            onChange={(e) => setFNewCat(e.target.value)}
            className={INPUT_CLS}
          />
        </label>
        <button
          disabled={busy || !fNewCat.trim()}
          className={BTN_SKY}
          onClick={() =>
            guard(async () => {
              await api.post("/api/v1/admin/forum-categories", {
                name: fNewCat.trim(),
              });
              setFNewCat("");
              setForumData(await api.get("/api/v1/admin/forums"));
            }, "已创建分区")
          }
        >
          新建分区
        </button>
      </div>
    </div>
  );
}
