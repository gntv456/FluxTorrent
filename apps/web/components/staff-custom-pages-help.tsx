"use client";

import { useI18n } from "@/i18n/client";

/** 帮助中心字段编辑器（0274）——从 staff-custom-pages 抽出以守 300 行门禁。
 *
 *  doc_group / doc_sort 是 custom_pages 的两个可空列（0274 迁移）：给任意
 *  自定义页填上 doc_group，它就会出现在前台 /help 目录里。留空 = 普通页。
 *  正文与展示仍走既有的 /p/{slug} 链路（ ammonia 消毒 + longform 排版）。
 *
 *  ⚠️ PUT 是全量覆盖：父组件保存时必须把这两个字段一起回传，否则会被清空
 *  导致该页从 /help 目录消失。 */

export interface HelpFieldsValue {
  doc_group: string | null;
  doc_sort: number | null;
}

const FLD =
  "min-h-[38px] w-full rounded-[var(--r-sm)] border " +
  "border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

export function HelpFieldsEditor({
  value,
  onChange,
}: {
  value: HelpFieldsValue;
  onChange: (v: HelpFieldsValue) => void;
}) {
  const { dict } = useI18n();
  return (
    <>
      <label className="text-xs text-sub">
        {dict.adminPages.docGroupLabel}
        <input
          className={FLD}
          value={value.doc_group ?? ""}
          onChange={(e) =>
            onChange({ ...value, doc_group: e.target.value || null })
          }
          placeholder="guide"
        />
      </label>
      <label className="flex items-center gap-2 text-xs text-sub">
        {dict.adminPages.docSortLabel}
        <input
          type="number"
          className={`${FLD} w-24`}
          value={value.doc_sort ?? ""}
          onChange={(e) =>
            onChange({
              ...value,
              doc_sort: e.target.value ? Number(e.target.value) : null,
            })
          }
        />
      </label>
    </>
  );
}

/** 列表里的分组徽标（表格列用）。 */
export function HelpGroupBadge({
  group,
  sort,
}: {
  group: string | null;
  sort: number | null;
}) {
  if (!group) return <span className="text-sub">—</span>;
  return (
    <span className="badge">
      {group}
      {sort !== null ? ` #${sort}` : ""}
    </span>
  );
}
