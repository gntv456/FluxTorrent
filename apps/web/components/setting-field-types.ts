/** 站点设定字段契约（从 components/setting-field.tsx 按域拆出）：
 *  schema 的 RSC 侧预取（/admin/settings page.tsx）与客户端渲染共用。 */

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
