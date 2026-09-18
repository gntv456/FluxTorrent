"use client";

/**
 * 站点设定字段组件（方案 §6）：按 meta.type 分发 9 类控件。
 * 统一规格：44px 触控高度、聚焦光晕、错误态红描边；错误内联展示不打断填写。
 */

import { useState } from "react";
import { useI18n } from "@/i18n/client";

export interface SettingFieldMeta {
  name: string;
  type: string;
  label: string;
  label_en: string | null;
  hint: string | null;
  unit: string | null;
  min: number | null;
  max: number | null;
  step: number | null;
  options: unknown;
  secret: boolean;
  readonly: boolean;
  writable: boolean;
  value: string;
  configured: boolean;
  updated_at: string;
}

export interface SettingCard {
  key: string;
  fields: SettingFieldMeta[];
}

export interface SettingGroup {
  key: string;
  label: string;
  writable: boolean;
  count: number;
  cards: SettingCard[];
}

export interface SettingsSchema {
  role: string;
  editable: boolean;
  field_count: number;
  group_count: number;
  groups: SettingGroup[];
}

interface EnumOption {
  v: string;
  l: string;
}

/** enum 的 options 是 [{v,l}] 数组；文本类字段的 options 是 {rule} 对象 */
function enumOptions(options: unknown): EnumOption[] {
  if (!Array.isArray(options)) return [];
  return options
    .filter(
      (o): o is { v: string; l: string } =>
        !!o && typeof o === "object" && "v" in o,
    )
    .map((o) => ({ v: String(o.v), l: String(o.l ?? o.v) }));
}

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
  const [reveal, setReveal] = useState(false);
  // 密文显示明文的二次确认（§6.3）
  const [askReveal, setAskReveal] = useState(false);
  const lock = disabled || field.readonly || !field.writable;
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
              // 已设置的密文：显示明文前二次确认
              if (field.secret && field.configured) {
                setAskReveal(true);
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
