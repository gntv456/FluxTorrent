"use client";

import { useI18n } from "@/i18n/client";
import type { PmBox } from "@/components/message-center-table";

/** 消息中心工具条（从 components/message-center.tsx 按域拆出）：
 *  ToolbarSwitcher 收件箱/自建文件夹/发件箱切换 + 🗂 管理 + 搜索/未读
 *  筛选 + 写信；BulkBar 全选/已读/移动/删除批量操作。
 *  状态与动作由 MessageCenter 注入。 */

export function ToolbarSwitcher({
  t,
  box,
  folder,
  boxes,
  search,
  setSearch,
  onlyUnread,
  setOnlyUnread,
  onShowInbox,
  onShowFolder,
  onShowSent,
  onToggleManage,
  onCompose,
}: {
  t: ReturnType<typeof useI18n>["dict"]["messages"];
  box: "inbox" | "sent";
  folder: number | null;
  boxes: PmBox[];
  search: string;
  setSearch: (v: string) => void;
  onlyUnread: boolean;
  setOnlyUnread: (v: boolean) => void;
  onShowInbox: () => void;
  onShowFolder: (id: number) => void;
  onShowSent: () => void;
  onToggleManage: () => void;
  onCompose: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={onShowInbox}
          aria-current={box === "inbox" ? "true" : undefined}
          className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
            box === "inbox"
              ? "border-[var(--baozi-orange)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white shadow-[0_5px_13px_var(--accent-shadow)]"
              : "border-[var(--baozi-line)] bg-[var(--row-veil-soft)] text-ink hover:-translate-y-px hover:text-[var(--baozi-orange)]"
          }`}
        >
          {t.inbox}
        </button>
        {boxes.map((b) => (
          <button
            key={b.id}
            type="button"
            onClick={() => onShowFolder(b.id)}
            aria-current={folder === b.id ? "true" : undefined}
            className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
              folder === b.id
                ? "border-[var(--baozi-orange)] bg-[var(--sky-soft)] text-[var(--baozi-orange-dark)]"
                : "border-[var(--baozi-line)] bg-[var(--row-veil-soft)] text-ink"
            }`}
          >
            {b.name} ({b.count})
          </button>
        ))}
        <button
          type="button"
          onClick={onShowSent}
          aria-current={box === "sent" ? "true" : undefined}
          className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
            box === "sent"
              ? "border-[var(--baozi-orange)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white"
              : "border-[var(--baozi-line)] bg-[var(--row-veil-soft)] text-ink"
          }`}
        >
          {t.sent}
        </button>
        <button
          type="button"
          onClick={onToggleManage}
          className="min-h-[44px] rounded-[10px] border border-[var(--baozi-line)] bg-[var(--row-veil-soft)] px-3 text-sm text-sub"
          title={t.manageBoxes}
        >
          🗂
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {box === "inbox" && (
          <>
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t.searchPh}
              className="min-h-[44px] w-40 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm"
            />
            <label className="flex min-h-[44px] items-center gap-1 text-xs text-sub">
              <input
                type="checkbox"
                checked={onlyUnread}
                onChange={(e) => setOnlyUnread(e.target.checked)}
              />
              {t.onlyUnread}
            </label>
          </>
        )}
        <button
          type="button"
          onClick={onCompose}
          className="min-h-[44px] rounded-[10px] border border-[var(--baozi-orange-dark)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-4 text-sm font-bold text-white shadow-[var(--shadow-hover)] active:scale-[0.97]"
        >
          {t.compose}
        </button>
      </div>
    </div>
  );
}

export function BulkBar({
  t,
  box,
  boxes,
  selected,
  allSelected,
  onToggleAll,
  busy,
  onMarkRead,
  onMove,
  onDelete,
}: {
  t: ReturnType<typeof useI18n>["dict"]["messages"];
  box: "inbox" | "sent";
  boxes: PmBox[];
  selected: number[];
  allSelected: boolean;
  onToggleAll: (on: boolean) => void;
  busy: boolean;
  onMarkRead: () => void;
  onMove: (folder: number | null) => void;
  onDelete: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2 text-xs">
      <label className="flex items-center gap-1 text-sub">
        <input
          type="checkbox"
          checked={allSelected}
          onChange={(e) => onToggleAll(e.target.checked)}
        />
        {t.selectAll}
      </label>
      {box === "inbox" && (
        <button
          type="button"
          disabled={selected.length === 0 || busy}
          onClick={onMarkRead}
          className="min-h-[36px] rounded-full border border-[var(--baozi-line)] px-3 font-bold disabled:opacity-40"
        >
          ✓ {t.markRead}
        </button>
      )}
      {box === "inbox" && boxes.length > 0 && (
        <select
          disabled={selected.length === 0 || busy}
          onChange={(e) => {
            const v = e.target.value;
            onMove(v === "" ? null : Number(v));
            e.target.value = "";
          }}
          className="min-h-[36px] rounded-full border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 font-bold disabled:opacity-40"
          defaultValue=""
        >
          <option value="">{t.moveTo}</option>
          <option value="">{t.moveToInbox}</option>
          {boxes.map((b) => (
            <option key={b.id} value={b.id}>
              {t.moveToBox.replace("{name}", b.name)}
            </option>
          ))}
        </select>
      )}
      <button
        type="button"
        disabled={selected.length === 0 || busy}
        onClick={onDelete}
        className="min-h-[36px] rounded-full border border-[var(--danger-border)] px-3 font-bold text-danger disabled:opacity-40"
      >
        🗑 {t.deleteSel}
      </button>
      <span className="text-sub">
        {t.selectedCount.replace("{n}", String(selected.length))}
      </span>
    </div>
  );
}
