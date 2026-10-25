"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import type { ForumCategory } from "@fluxtorrent/domain-types";
import {
  INPUT_CLS,
  NO_CATEGORY,
  catOf,
  type ForumAdminForum,
} from "./forum-structure-types";
import { useCategoryDnd } from "./use-category-dnd";
import { ForumCatCreate } from "./forum-cat-create";

/** 左栏：分区树（对齐样图「论坛分区」侧卡）。
 *
 *  - 条目两行：分区名 + 「N 个版块 · N 主题」；选中项 sky-soft 底色
 *  - 行内操作（拖动/上下移/显隐/改名/删除）平时隐藏，hover / 键盘聚焦 /
 *    选中时显现 —— 样图侧栏是纯列表，操作不能丢，只能收起来
 *  - ↑↓ 调序与拖拽共用同一状态机（批量 reorder，单事务落库）
 *  - 底部「＋ 新建分区」开关式展开输入框（样图按钮位）
 *  - 「未分组」是伪条目：承载 category_id 为空的版块（外键 ON DELETE
 *    SET NULL，删分区不会留悬空引用，版块只会掉到这里）
 */
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
  const [editing, setEditing] = useState<number | null>(null);
  const [editName, setEditName] = useState("");
  // P2 清偿：删分区二次确认（danger），window.confirm 退役
  const ask = useConfirm();
  // 排序状态机（↑↓ 与拖拽共用，含乐观更新与失败回滚）
  const dnd = useCategoryDnd(categories, run);

  const orphans = forums.filter(
    (x) => typeof x.category_id !== "number",
  );
  /** 某分区下全部版块的主题数之和（左栏副行用） */
  const topicsOf = (cid: number) =>
    forums.reduce(
      (s, x) => (catOf(x) === cid ? s + x.topics : s),
      0,
    );

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
    run(
      async () => {
        await api.put(`/api/v1/admin/forum-categories/${c.id}`, {
          name: c.name,
          sort: c.sort,
          visible: !c.visible,
        });
      },
      c.visible ? f.catHidden : f.catShown,
    );
  }

  function remove(c: ForumCategory) {
    const n = forums.filter((x) => x.category_id === c.id).length;
    const tail = `\n\n${f.delConfirmTail}`;
    const hint =
      n > 0
        ? `${fmt(f.catDelNonEmpty, { name: c.name, n })}${tail}`
        : `${fmt(f.catDelEmpty, { name: c.name })}${tail}`;
    ask.open({
      title: f.delBtn,
      message: hint,
      tone: "danger",
      busy,
      run: () =>
        run(async () => {
          await api.del(`/api/v1/admin/forum-categories/${c.id}`);
        }, f.catDeleted),
    });
  }

  function create(name: string) {
    run(async () => {
      await api.post("/api/v1/admin/forum-categories", {
        name,
        sort: categories.length,
      });
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
  const CURSOR = " cursor-grab active:cursor-grabbing";
  /** 行内操作平时隐身；选中 / 编辑 / hover / 键盘聚焦时显现 */
  const reveal = (active: boolean) =>
    active
      ? ""
      : "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100";

  return (
    <aside className="brand-panel w-[248px] shrink-0">
      <div className="border-b border-line px-3.5 py-2.5 text-[13px]
        font-bold text-ink">
        {f.secLabel}
      </div>

      <div className="p-2">
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
                "group " +
                (selected === c.id ? rowOn : rowOff) +
                (editing === c.id ? "" : CURSOR) +
                (dnd.dragIdx === i ? " opacity-40" : "")
              }
            >
              <span
                className={`${GRIP} ${reveal(editing === c.id)}`}
                aria-hidden="true"
              >
                ⋮⋮
              </span>
              <button
                type="button"
                className="min-w-0 flex-1 text-left"
                onClick={() => onSelect(c.id)}
                title={c.name}
              >
                <span
                  className={`block truncate text-[12.5px] font-semibold ${
                    c.visible ? "text-ink" : "text-sub line-through"
                  }`}
                >
                  {c.name}
                </span>
                <span className="block truncate text-xs text-sub">
                  {fmt(f.catSub, {
                    b: c.forums,
                    t: topicsOf(c.id),
                  })}
                </span>
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
                <span
                  className={
                    "inline-flex shrink-0 items-center gap-0.5 " +
                    reveal(selected === c.id)
                  }
                >
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
                </span>
              )}
            </li>
          ))}

          {orphans.length > 0 && (
            <li className={selected === NO_CATEGORY ? rowOn : rowOff}>
              <button
                type="button"
                className="min-w-0 flex-1 text-left"
                onClick={() => onSelect(NO_CATEGORY)}
              >
                <span className="block truncate text-[12.5px] font-semibold
                  text-sub">
                  {f.noCategory}
                </span>
                <span className="block truncate text-xs text-sub">
                  {fmt(f.catSub, {
                    b: orphans.length,
                    t: topicsOf(NO_CATEGORY),
                  })}
                </span>
              </button>
            </li>
          )}
        </ul>

        {/* 底部新建分区：样图为全宽幽灵按钮，点击展开输入行 */}
        <ForumCatCreate busy={busy} onCreate={create} />
      </div>
      <AskDialog {...ask} />
    </aside>
  );
}
