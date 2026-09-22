"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { ForumCategory } from "@fluxtorrent/domain-types";
import {
  BTN_LINE, BTN_SKY, INPUT_CLS, NO_CATEGORY,
  type ForumAdminForum,
} from "./forum-structure-types";
import { useCategoryDnd } from "./use-category-dnd";

const TIP_CLS =
  "mt-3 rounded-[var(--r-sm)] bg-sun-soft p-2 text-xs " +
  "leading-relaxed text-ink";

/** 左栏：分区树。
 *  - 点击选中（右栏显示该分区下的版块）
 *  - ↑↓ 调序（批量 reorder，单事务落库）
 *  - 👁 切换前台可见性（隐藏后前台整块消失，后台仍可见）
 *  - ✎ 改名 / × 删除（删除只解绑版块，不删版块）
 *  - 底部新建分区
 *  「未分组」是伪条目：承载 category_id 为空的版块（外键 ON DELETE SET NULL，
 *  所以删分区不会留下悬空引用，版块只会掉到这里）。 */
export function ForumCategoryPanel({
  categories,
  forums,
  selected,
  onSelect,
  busy,
  run,
}: {
  categories: ForumCategory[];
  forums: ForumAdminForum[];
  selected: number;
  onSelect: (id: number) => void;
  busy: boolean;
  run: (fn: () => Promise<void>, ok: string) => void;
}) {
  const { dict } = useI18n();
  const f = dict.adminForums;
  const [newName, setNewName] = useState("");
  const [editing, setEditing] = useState<number | null>(null);
  const [editName, setEditName] = useState("");
  // 排序状态机（↑↓ 与拖拽共用，含乐观更新与失败回滚）
  const dnd = useCategoryDnd(categories, run);

  const orphans = forums.filter(
    (f) => typeof f.category_id !== "number",
  ).length;

  function rename(c: ForumCategory) {
    const name = editName.trim();
    setEditing(null);
    if (!name || name === c.name) return;
    run(async () => {
      await api.put(`/api/v1/admin/forum-categories/${c.id}`, {
        name,
        sort: c.sort,
        visible: c.visible,
      });
    }, f.catRenamed);
  }

  function toggleVisible(c: ForumCategory) {
    run(async () => {
      await api.put(`/api/v1/admin/forum-categories/${c.id}`, {
        name: c.name,
        sort: c.sort,
        visible: !c.visible,
      });
    }, c.visible ? f.catHidden : f.catShown);
  }

  function remove(c: ForumCategory) {
    const n = forums.filter((x) => x.category_id === c.id).length;
    const hint =
      n > 0
        ? `${fmt(f.catDelNonEmpty, { name: c.name, n })}\n\n${f.delConfirmTail}`
        : `${fmt(f.catDelEmpty, { name: c.name })}\n\n${f.delConfirmTail}`;
    if (!window.confirm(hint)) return;
    run(async () => {
      await api.del(`/api/v1/admin/forum-categories/${c.id}`);
    }, f.catDeleted);
  }

  function create() {
    const name = newName.trim();
    if (!name) return;
    run(async () => {
      await api.post("/api/v1/admin/forum-categories", {
        name,
        sort: categories.length,
      });
      setNewName("");
    }, f.catCreated);
  }

  const rowOn =
    "flex items-center gap-0.5 rounded-[var(--r-sm)] bg-sky-soft " +
    "px-1.5 py-1.5 border border-transparent";
  const rowOff =
    "flex items-center gap-0.5 rounded-[var(--r-sm)] px-1.5 py-1.5 " +
    "border border-transparent hover:bg-cloud";
  /** 拖拽手柄 + 被拖行淡出，是「可拖动」唯一的视觉线索（面板太窄放不下独立列） */
  const GRIP = "shrink-0 select-none text-[10px] leading-none text-sub";

  return (
    <aside className="w-[248px] shrink-0 border-r border-line pr-3">
      <div className="mb-2 flex items-center justify-between text-sm">
        <b>{f.secLabel}</b>
        <span className="text-xs text-sub">
          {fmt(f.countSuffix, { n: categories.length })}
        </span>
      </div>

      <ul className="flex flex-col gap-0.5">
        {dnd.list.map((c, i) => (
          <li
            key={c.id}
            // 编辑态不拖：draggable 会抢走 input 里的文本选择
            draggable={!busy && editing !== c.id}
            onDragStart={(e) => {
              // Firefox 必须设 dataTransfer 才肯开始拖
              e.dataTransfer.effectAllowed = "move";
              e.dataTransfer.setData("text/plain", String(c.id));
              dnd.startDrag(i);
            }}
            onDragOver={(e) => {
              e.preventDefault(); // 不拦默认行为就收不到 drop，光标也是禁止
              dnd.overDrag(i);
            }}
            onDragEnd={dnd.endDrag}
            title={f.dragTip}
            className={
              (selected === c.id ? rowOn : rowOff) +
              (editing === c.id ? "" : " cursor-grab active:cursor-grabbing") +
              (dnd.dragIdx === i ? " opacity-40" : "")
            }
          >
            <span className={GRIP} aria-hidden="true">
              ⋮⋮
            </span>
            <button
              type="button"
              className="min-w-0 flex-1 text-left text-sm"
              onClick={() => onSelect(c.id)}
              title={c.name}
            >
              <span className={c.visible ? "" : "text-sub line-through"}>
                {c.name}
              </span>
              <span className="ml-1.5 text-xs text-sub">{c.forums}</span>
            </button>
            {editing === c.id ? (
              <input
                autoFocus
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                onBlur={() => rename(c)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") rename(c);
                  if (e.key === "Escape") setEditing(null);
                }}
                className={`${INPUT_CLS} min-h-[26px] w-24 text-xs`}
              />
            ) : (
              <>
                <button
                  type="button"
                  disabled={busy}
                  title={f.moveUp}
                  className="shrink-0 text-xs text-sub hover:text-sky"
                  onClick={() => dnd.move(i, -1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  disabled={busy}
                  title={f.moveDown}
                  className="shrink-0 text-xs text-sub hover:text-sky"
                  onClick={() => dnd.move(i, 1)}
                >
                  ↓
                </button>
                <button
                  type="button"
                  disabled={busy}
                  title={c.visible ? f.visibleTip : f.hiddenTip}
                  className="shrink-0 text-xs"
                  onClick={() => toggleVisible(c)}
                >
                  {c.visible ? "👁" : "🚫"}
                </button>
                <button
                  type="button"
                  disabled={busy}
                  title={f.renameTip}
                  className="shrink-0 text-xs text-sub hover:text-sky"
                  onClick={() => {
                    setEditing(c.id);
                    setEditName(c.name);
                  }}
                >
                  ✎
                </button>
                <button
                  type="button"
                  disabled={busy}
                  title={f.delTip}
                  className="shrink-0 text-xs font-bold text-danger"
                  onClick={() => remove(c)}
                >
                  ×
                </button>
              </>
            )}
          </li>
        ))}

        {orphans > 0 && (
          <li
            className={
              selected === NO_CATEGORY ? rowOn : rowOff
            }
          >
            <button
              type="button"
              className="min-w-0 flex-1 text-left text-sm text-sub"
              onClick={() => onSelect(NO_CATEGORY)}
            >
              {f.noCategory}
              <span className="ml-1.5 text-xs">{orphans}</span>
            </button>
          </li>
        )}
      </ul>

      <div className="mt-3 flex items-center gap-1">
        <input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") create();
          }}
          placeholder={f.newNamePh}
          className={`${INPUT_CLS} min-h-[32px] min-w-0 text-xs`}
        />
        <button
          type="button"
          disabled={busy || !newName.trim()}
          className={`${BTN_SKY} min-h-[32px] shrink-0 px-3 text-xs`}
          onClick={create}
        >
          {f.createBtn}
        </button>
      </div>

      <p className={TIP_CLS}>{f.catTip}</p>

      <p className="mt-2 text-xs text-sub">
        <button
          type="button"
          className={`${BTN_LINE} min-h-[28px] w-full text-xs`}
          onClick={() => window.location.assign("/forums")}
        >
          {f.previewBtn}
        </button>
      </p>
    </aside>
  );
}
