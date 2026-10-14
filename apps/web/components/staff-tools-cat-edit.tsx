"use client";

/**
 * 分类编辑对话框（P2 清偿）：分类行「编辑」原是 6 连 window.prompt 链
 * （改名/iconKey/bgColor/父类/排序/newznab）+ 1 个 alert 兜底——收敛为
 * 单对话框六字段草稿，一次确认一次 PUT。旧链语义保留：空串 sort/newznab
 * = 不改（undefined）、父类 0 = 顶级。
 */

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import type { CatItem } from "./staff-tools-content-shared";

/** 六字段草稿（parent/sort/nz 以字符串编辑，提交时换算） */
function draftOf(c: CatItem) {
  return {
    name: c.name,
    icon: c.icon_key ?? "",
    bg: c.bg_color ?? "",
    parent: c.parent_id ? String(c.parent_id) : "0",
    sort: String(c.sort),
    nz: c.newznab_id != null ? String(c.newznab_id) : "",
  };
}

export function StaffCatEditDialog({
  cat,
  onClose,
  busy,
  guard,
}: {
  cat: CatItem;
  onClose: () => void;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [d, setD] = useState(draftOf(cat));

  // 字段顺序即旧 prompt 链顺序；label 全部复用既有三语键
  const fields: [keyof typeof d, string][] = [
    ["name", t.renamePrompt],
    ["icon", t.iconKeyPrompt],
    ["bg", t.bgColorPrompt],
    ["parent", dict.cmgmt.parentPrompt],
    ["sort", t.sortPrompt],
    ["nz", t.newznabPrompt],
  ];

  function save() {
    const nzT = d.nz.trim();
    // 非法输入（非数字）必须拦下：Number(NaN) || 0 会把手滑打成 0 =
    // 静默清除映射（旧 alert → toast.warn；不关对话框，可就地改）
    if (nzT !== "" && !/^\d+$/.test(nzT)) {
      toast.warn(t.newznabInvalid);
      return;
    }
    void guard(async () => {
      await api.put(`/api/v1/admin/categories/${cat.id}`, {
        name: d.name,
        icon_key: d.icon.trim(),
        bg_color: d.bg.trim(),
        parent_id: Number(d.parent) || null,
        sort: d.sort.trim() === "" ? undefined : Number(d.sort),
        newznab_id: nzT === "" ? undefined : Number(nzT),
      });
    }, t.saved);
    onClose();
  }

  return (
    <ConfirmDialog
      open
      onClose={onClose}
      onConfirm={save}
      tone="neutral"
      title={dict.cmgmt.btnEdit}
      busy={busy}
    >
      <div className="flex flex-col gap-2">
        {fields.map(([k, label]) => (
          <label key={k} className="flex flex-col gap-1 text-sm">
            <span>{label}</span>
            <input
              value={d[k]}
              onChange={(e) => setD({ ...d, [k]: e.target.value })}
              data-autofocus={k === "name" ? true : undefined}
              className="min-h-[36px] rounded-[var(--r-sm)] border
                border-line bg-[var(--surface-card)] px-3 text-sm"
            />
          </label>
        ))}
      </div>
    </ConfirmDialog>
  );
}
