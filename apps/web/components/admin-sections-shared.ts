/**
 * 维度管理·共享类型与常量（从 components/admin-sections.tsx 按域拆出）：
 * 分类模式/维度字典/分类行类型、兜底维度表与七维开关表。
 */

export interface ModeRow {
  id: number;
  name: string;
  show_source: boolean;
  show_medium: boolean;
  show_codec: boolean;
  show_audio_codec: boolean;
  show_standard: boolean;
  show_processing: boolean;
  show_team: boolean;
  categories: number;
  /** B2（0195）：mode_kinds 关联表下的**可见维度**清单（权威来源） */
  visible_kinds?: string[] | null;
  /** B2：被显式关闭的维度清单 */
  hidden_kinds?: string[] | null;
}

export interface DictRow {
  id: number;
  kind: string;
  name: string;
  sort: number;
  mode_id: number | null;
}

export interface CategoryRow {
  id: number;
  name: string;
  torrents: number;
  mode_id: number | null;
  auto_approve?: boolean;
}

export interface SectionKindMeta {
  kind: string;
  label: string;
  sort: number;
  /** B2（0195）六类型；缺省（旧后端）按 select 处理 */
  field_type?: string;
  required?: boolean;
  multiple?: boolean;
  enabled?: boolean;
  icon_key?: string | null;
  bg_color?: string | null;
  /** 已挂字典项数（枚举维度用） */
  options?: number;
}

/** B2 六类型字段系统的显示名键（i18n `adminSections.fieldTypes`）与取值集合 */
export const FIELD_TYPE_KEYS = [
  "text",
  "number",
  "select",
  "multiselect",
  "date",
  "bool",
] as const;

/** 兜底维度表：取值键在 `KIND_KEYS`，显示名在 i18n `adminSections.kinds`（三语），
 *  由 `fallbackKinds()` 拼回原 `SectionKindMeta[]` 形状（sort 保持 10/20/…/90）。 */
export const KIND_KEYS = [
  "media", "grades", "editions", "codec", "audio_codec",
  "standard", "team", "source", "processing",
] as const;

export const fallbackKinds = (
  labels?: Record<string, string>,
): SectionKindMeta[] =>
  KIND_KEYS.map((kind, i) => ({
    kind,
    label: labels?.[kind] ?? kind,
    sort: (i + 1) * 10,
  }));

export const FLAGS: [keyof ModeRow, string][] = [
  ["show_source", "Source"],
  ["show_medium", "Media"],
  ["show_codec", "Codec"],
  ["show_audio_codec", "Audio"],
  ["show_standard", "Standard"],
  ["show_processing", "Processing"],
  ["show_team", "Team"],
];

/** 维度/字典表单输入框统一样式 */
export const FIELD_INPUT_CLS =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line px-2 text-sm";
/** 字典维度下拉（带卡片底色） */
export const SELECT_FIELD_CLS =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2";
/** 归属模式下拉（紧凑 32px） */
export const MODE_SELECT_CLS =
  "min-h-[32px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-1";
