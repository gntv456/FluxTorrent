"use client";

/**
 * 站点设定字段组件（方案 §6）：按 meta.type 分发 9 类控件。
 * 统一规格：44px 触控高度、聚焦光晕、错误态红描边；错误内联展示不打断填写。
 * 类型契约拆出 @/components/setting-field-types；
 * 9 类控件渲染拆出 @/components/setting-field-controls。
 */

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import {
  enumOptions,
  useFieldControl,
} from "@/components/setting-field-controls";
import type { SettingFieldMeta } from "@/components/setting-field-types";

export type {
  SettingFieldMeta,
  SettingCard,
  SettingGroup,
  SettingsSchema,
} from "@/components/setting-field-types";

export function SettingField({
  field,
  value,
  error,
  disabled,
  onChange,
  onBlur,
  onHistory,
}: {
  field: SettingFieldMeta;
  value: string;
  error?: string;
  disabled: boolean;
  onChange: (v: string) => void;
  onBlur: () => void;
  onHistory?: () => void;
}) {
  const { dict } = useI18n();
  const s = dict.settingsAdmin;
  // 密文显示明文的二次确认（§6.3）
  const [askReveal, setAskReveal] = useState(false);
  // 控件与可见 label 的关联 id（a11y：label/select 需有可访问名称）
  const cid = `sf-${field.name}`;

  const base =
    "min-h-[44px] rounded-[var(--r-sm)] border bg-[var(--surface-card)] px-3 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-sky/40 disabled:bg-cloud disabled:text-sub";
  const border = error ? "border-danger" : "border-line";
  const inputCls = `${base} ${border} w-full`;

  // 开关类控件（yesno / ≤4 项 enum）无错误时走紧凑布局：去掉独立卡片容器，
  // 标签与按钮同行 —— 上层用网格多列排布（模块开关一屏可见十几项而非十几屏）
  const compact =
    !error &&
    !askReveal &&
    (field.type === "yesno" ||
      (field.type === "enum" &&
        enumOptions(field.options).length > 0 &&
        enumOptions(field.options).length <= 4));

  const { control, reveal, setReveal } = useFieldControl({
    field,
    value,
    error,
    disabled,
    onChange,
    onBlur,
    inputCls,
    base,
    border,
    cid,
  });

  // 紧凑模式：标签与开关同行（去掉独立卡片容器），由上层网格多列排布
  if (compact) {
    return (
      <div
        className={`flex items-center gap-2 rounded-[var(--r-sm)] border p-2 ${
          error ? "border-danger bg-danger/5" : "border-line bg-[var(--surface-card)]"
        }`}
      >
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-bold text-ink" title={field.label}>
            {field.label}
            {field.readonly && (
              <span className="ml-1.5 rounded-full bg-sky-soft px-1.5 py-0.5 text-[10px] font-bold text-sub">
                {s.readonlyField}
              </span>
            )}
          </p>
          {field.hint && (
            <p className="truncate text-[10px] leading-tight text-sub" title={field.hint}>
              {field.hint}
            </p>
          )}
        </div>
        {control}
        {onHistory && (
          <button
            type="button"
            onClick={onHistory}
            title={s.history}
            className="shrink-0 text-[10px] font-bold text-sub underline hover:text-ink"
          >
            史
          </button>
        )}
      </div>
    );
  }

  return (
    <div
      className={`rounded-[var(--r-sm)] border p-3 ${
        error ? "border-danger bg-danger/5" : "border-line bg-[var(--surface-card)]"
      }`}
    >
      <div className="flex flex-wrap items-baseline gap-2">
        <label htmlFor={cid} className="text-sm font-bold text-ink">
          {field.label}
        </label>
        <span className="font-mono text-[11px] text-sub">{field.name}</span>
        {field.readonly && (
          <span className="rounded-full bg-sky-soft px-2 py-0.5 text-[10px] font-bold text-sub">
            {s.readonlyField}
          </span>
        )}
        <span className="flex-1" />
        {onHistory && (
          <button
            type="button"
            onClick={onHistory}
            className="text-[11px] font-bold text-sub underline hover:text-ink"
          >
            {s.history}
          </button>
        )}
      </div>
      <div className="mt-2">{control}</div>
      {askReveal && (
        <div className="mt-2 flex flex-wrap items-center gap-2 rounded-[var(--r-sm)] border border-sun/60 bg-sun/10 p-2">
          <p className="text-[11px] font-bold text-ink">{s.revealConfirm}</p>
          <span className="flex-1" />
          <button
            type="button"
            onClick={() => {
              setReveal(true);
              setAskReveal(false);
            }}
            className="min-h-[44px] rounded-full bg-sky-deep px-3 text-[11px] font-bold text-white"
          >
            {s.revealConfirmYes}
          </button>
          <button
            type="button"
            onClick={() => setAskReveal(false)}
            className="min-h-[44px] rounded-full border border-line bg-[var(--surface-card)] px-3 text-[11px] font-bold text-sub"
          >
            {s.cancel}
          </button>
        </div>
      )}
      {field.hint && <p className="mt-1.5 text-[11px] leading-snug text-sub">{field.hint}</p>}
      {field.type === "pair" && (
        <p className="mt-1.5 text-[11px] leading-snug text-sub">{s.pairHint}</p>
      )}
      {field.secret && !error && (
        <p className="mt-1.5 text-[11px] leading-snug text-sub">
          {field.configured ? s.secretSet : s.unset} · {s.secretKeep}
        </p>
      )}
      {error && <p className="mt-1.5 text-[11px] font-bold text-danger">{error}</p>}
    </div>
  );
}
