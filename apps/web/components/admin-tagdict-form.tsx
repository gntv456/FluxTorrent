"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useState } from "react";

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

import { EMPTY, type TagRow } from "./admin-tagdict-shared";

/** 第八轮 P1-2：标签管理（从 components/admin-tagdict.tsx 按域拆出）：
 *  新建/编辑标签表单——名称/类型/作用域 + 样式属性
 *  （背景/字体色/字号/边距/圆角）+ 分类模式作用域 + 实时预览。 */

// 表单输入框底色 / 取消按钮
const INP =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-2 text-sm outline-none focus:border-sky";
const BTN_CANCEL =
  BTN_SM_BOLD;
/** 预览底色：标签透明背景时衬一块中性底，否则暗色主题下白字标签贴着暗底看不见 */
const PREVIEW_SHELL =
  "inline-block rounded-[var(--r-sm)] bg-[var(--surface-card)] " + "px-2 py-1";

export type TagEdit = { id: number | null; f: Omit<TagRow, "id"> };

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1 text-xs">
      {label}
      {children}
    </label>
  );
}

export function TagEditForm({
  edit,
  setEdit,
  modes,
  busy,
  save,
}: {
  edit: TagEdit;
  setEdit: React.Dispatch<React.SetStateAction<TagEdit>>;
  modes: { id: number; name: string }[];
  busy: boolean;
  save: () => void;
}) {
  const { dict } = useI18n();
  const at = dict.adminTagdict;

  const set = <K extends keyof Omit<TagRow, "id">>(
    k: K,
    v: Omit<TagRow, "id">[K],
  ) => setEdit({ ...edit, f: { ...edit.f, [k]: v } });

  return (
    <section className="baozi-panel cmgmt-form p-4">
      <h2 className="mb-2 text-base font-bold">
        {edit.id === null ? at.formNew : fmt(at.formEdit, { id: edit.id })}
      </h2>
      <div className="flex flex-wrap items-end gap-2">
        <Field label={at.fName}>
          <input
            value={edit.f.name}
            onChange={(e) => set("name", e.target.value)}
            className={`${INP} w-28`}
          />
        </Field>
        <Field label={at.fKind}>
          <select
            value={edit.f.kind}
            onChange={(e) => set("kind", e.target.value)}
            className={INP}
          >
            <option value="plain">{at.kindNormal}</option>
            <option value="official">{at.kindOfficial}</option>
          </select>
        </Field>
        <Field label={at.fScope}>
          <select
            value={edit.f.scope ?? "torrent"}
            onChange={(e) => set("scope", e.target.value)}
            className={INP}
          >
            <option value="torrent">{at.scopeTorrent}</option>
            <option value="forum">{at.scopeForum}</option>
          </select>
        </Field>
        {/* 0160 P2：分组（发布/筛选按组分区）+ 层级（global 不随站型重建） */}
        <Field label={at.fGroup}>
          <select
            value={edit.f.tag_group ?? "attribute"}
            onChange={(e) => set("tag_group", e.target.value)}
            className={INP}
          >
            <option value="attribute">{at.optAttribute}</option>
            <option value="content">{at.optContent}</option>
          </select>
        </Field>
        <Field label={at.fLayer}>
          <select
            value={edit.f.scope_layer ?? "pack"}
            onChange={(e) => set("scope_layer", e.target.value)}
            className={INP}
            title={at.layerHint}
          >
            <option value="global">{at.layerGlobal}</option>
            <option value="pack">{at.layerSite}</option>
          </select>
        </Field>
        <Field label={at.fBgColor}>
          <input
            value={edit.f.bg_color}
            onChange={(e) => set("bg_color", e.target.value)}
            placeholder="#ff0000"
            className={`${INP} w-24`}
          />
        </Field>
        <Field label={at.fColor}>
          <input
            value={edit.f.color}
            onChange={(e) => set("color", e.target.value)}
            className={`${INP} w-24`}
          />
        </Field>
        <Field label={at.fFontSize}>
          <input
            value={edit.f.font_size}
            onChange={(e) => set("font_size", e.target.value)}
            className={`${INP} w-20`}
          />
        </Field>
        <Field label={at.fMargin}>
          <input
            value={edit.f.margin}
            onChange={(e) => set("margin", e.target.value)}
            className={`${INP} w-28`}
          />
        </Field>
        <Field label={at.fPadding}>
          <input
            value={edit.f.padding}
            onChange={(e) => set("padding", e.target.value)}
            className={`${INP} w-24`}
          />
        </Field>
        <Field label={at.fRadius}>
          <input
            value={edit.f.border_radius}
            onChange={(e) => set("border_radius", e.target.value)}
            className={`${INP} w-20`}
          />
        </Field>
        <Field label={at.fMode}>
          <select
            value={edit.f.mode_id ?? ""}
            onChange={(e) =>
              set("mode_id", e.target.value ? Number(e.target.value) : null)
            }
            className={INP}
          >
            <option value="">{at.optAllModes}</option>
            {modes.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
          </select>
        </Field>
        <Field label={at.fSort}>
          <input
            type="number"
            value={edit.f.sort}
            onChange={(e) => set("sort", Number(e.target.value))}
            className={`${INP} w-16`}
          />
        </Field>
        <label className="flex items-center gap-1 pb-2 text-xs">
          <input
            type="checkbox"
            checked={edit.f.enabled}
            onChange={(e) => set("enabled", e.target.checked)}
          />
          {at.enabled}
        </label>
        <button
          className="baozi-button"
          disabled={busy || !edit.f.name.trim()}
          onClick={save}
        >
          {at.save}
        </button>
        {edit.id !== null && (
          <button
            className={BTN_CANCEL}
            onClick={() => setEdit({ id: null, f: { ...EMPTY } })}
          >
            {at.cancel}
          </button>
        )}
      </div>
      {/* 预览：name 为空时也显示占位字样，样式改了立刻能看到效果 */}
      <p className="mt-3 flex items-center gap-2 text-xs text-sub">
        {at.previewLabel}
        <span className={PREVIEW_SHELL}>
          <span
            style={{
              background: edit.f.bg_color || "transparent",
              color: edit.f.color,
              fontSize: edit.f.font_size,
              margin: edit.f.margin,
              padding: edit.f.padding,
              borderRadius: edit.f.border_radius,
              border: edit.f.bg_color ? undefined : "1px solid #ccc",
            }}
          >
            {edit.f.name || at.previewPh}
          </span>
        </span>
      </p>
    </section>
  );
}
