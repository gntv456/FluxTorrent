"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import type { SettingFieldMeta } from "@/components/setting-field-types";

/** 设定字段 9 类控件分发（从 components/setting-field.tsx 按域拆出）：
 *  文本/密码/数字/开关(yesno·enum≤4)/下拉/等级/颜色/多行/键值对。
 *  紧凑布局判定与外壳渲染（卡片容器/标签/历史入口）留在原文件。 */

export interface EnumOption {
  v: string;
  l: string;
}

/** enum 的 options 是 [{v,l}] 数组；文本类字段的 options 是 {rule} 对象 */
export function enumOptions(options: unknown): EnumOption[] {
  if (!Array.isArray(options)) return [];
  return options
    .filter(
      (o): o is { v: string; l: string } =>
        !!o && typeof o === "object" && "v" in o,
    )
    .map((o) => ({ v: String(o.v), l: String(o.l ?? o.v) }));
}

/** 控件统一规格：44px 触控高度、聚焦光晕、错误态红描边 */
export function useFieldControl({
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
}: {
  field: SettingFieldMeta;
  value: string;
  error?: string;
  disabled: boolean;
  onChange: (v: string) => void;
  onBlur: () => void;
  inputCls: string;
  base: string;
  border: string;
  cid: string;
}) {
  const { dict } = useI18n();
  const s = dict.settingsAdmin;
  const [reveal, setReveal] = useState(false);
  const lock = disabled || field.readonly || !field.writable;

  let control: React.ReactNode;
  switch (field.type) {
    case "yesno":
      control = (
        <div className="flex gap-1.5" role="radiogroup" aria-label={field.label}>
          {(["yes", "no"] as const).map((v) => {
            const on = value === v;
            return (
              <button
                key={v}
                type="button"
                role="radio"
                aria-checked={on}
                disabled={lock}
                onClick={() => onChange(v)}
                className={`min-h-[32px] min-w-[56px] rounded-full px-3 text-xs font-bold ${
                  on
                    ? "bg-sky-deep text-white"
                    : "border border-line bg-[var(--surface-card)] text-sub hover:text-ink"
                }`}
              >
                {v === "yes" ? s.yes : s.no}
              </button>
            );
          })}
        </div>
      );
      break;
    case "enum": {
      const opts = enumOptions(field.options);
      control =
        opts.length > 0 && opts.length <= 4 ? (
          <div className="flex flex-wrap gap-1.5" role="radiogroup" aria-label={field.label}>
            {opts.map((o) => {
              const on = value === o.v;
              return (
                <button
                  key={o.v}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  disabled={lock}
                  onClick={() => onChange(o.v)}
                  className={`min-h-[32px] rounded-full px-3 text-xs font-bold ${
                    on
                      ? "bg-sky-deep text-white"
                      : "border border-line bg-[var(--surface-card)] text-sub hover:text-ink"
                  }`}
                >
                  {o.l}
                </button>
              );
            })}
          </div>
        ) : (
          <select
            id={cid}
            value={value}
            disabled={lock}
            onChange={(e) => onChange(e.target.value)}
            onBlur={onBlur}
            className={`${inputCls} max-w-md`}
          >
            <option value="">{s.selectPlaceholder}</option>
            {opts.map((o) => (
              <option key={o.v} value={o.v}>
                {o.l}
              </option>
            ))}
          </select>
        );
      break;
    }
    case "classlevel":
      control = (
        <select
          id={cid}
          value={value}
          disabled={lock}
          onChange={(e) => onChange(e.target.value)}
          onBlur={onBlur}
          className={`${inputCls} max-w-xs`}
        >
          {dict.admin.classList.map(([id, label]) => (
            <option key={id} value={id}>
              {label}（{id}）
            </option>
          ))}
        </select>
      );
      break;
    case "color":
      control = (
        <div className="flex items-center gap-2">
          <input
            type="color"
            aria-label={s.pickColor}
            disabled={lock}
            value={/^#[0-9a-fA-F]{6}$/.test(value) ? value : "#000000"}
            onChange={(e) => onChange(e.target.value)}
            className="h-[44px] w-[56px] cursor-pointer rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] p-1"
          />
          <input
            id={cid}
            value={value}
            disabled={lock}
            onChange={(e) => onChange(e.target.value)}
            onBlur={onBlur}
            placeholder="#FFD700"
            className={`${base} ${border} w-40 font-mono`}
          />
        </div>
      );
      break;
    case "password":
      control = (
        <div className="flex items-center gap-2">
          <input
            id={cid}
            type={reveal ? "text" : "password"}
            value={value}
            disabled={lock}
            onChange={(e) => onChange(e.target.value)}
            onBlur={onBlur}
            placeholder={field.configured ? "••••••••" : s.unset}
            autoComplete="new-password"
            className={`${inputCls} max-w-md`}
          />
          <button
            type="button"
            onClick={() => {
              if (reveal) {
                setReveal(false);
                return;
              }
              setReveal(true);
            }}
            disabled={lock}
            className="min-h-[44px] rounded-full border border-line bg-[var(--surface-card)] px-3 text-xs font-bold text-sub"
          >
            {reveal ? s.hide : s.show}
          </button>
        </div>
      );
      break;
    case "textarea":
      control = (
        <textarea
          id={cid}
          value={value}
          disabled={lock}
          rows={3}
          onChange={(e) => onChange(e.target.value)}
          onBlur={onBlur}
          className={`${inputCls} py-2 leading-relaxed`}
        />
      );
      break;
    case "number":
      control = (
        <div className="flex items-center gap-2">
          <input
            id={cid}
            type="number"
            value={value}
            disabled={lock}
            min={field.min ?? undefined}
            max={field.max ?? undefined}
            step={field.step ?? 1}
            onChange={(e) => onChange(e.target.value)}
            onBlur={onBlur}
            className={`${base} ${border} num w-44`}
          />
          {field.unit && <span className="text-xs text-sub">{field.unit}</span>}
        </div>
      );
      break;
    default:
      // text / pair
      control = (
        <input
          id={cid}
          value={value}
          disabled={lock}
          onChange={(e) => onChange(e.target.value)}
          onBlur={onBlur}
          className={`${inputCls} max-w-2xl`}
        />
      );
  }
  return { control, reveal, setReveal };
}
