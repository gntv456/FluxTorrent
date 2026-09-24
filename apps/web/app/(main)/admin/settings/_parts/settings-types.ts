/**
 * 站点设定共享类型（从 app/(main)/admin/settings/settings-client.tsx 按域拆出）：
 * 修改历史行 / 保存结果 / 导出导入载荷。仅类型，无运行时代码。
 */

export interface HistoryRow {
  id: number;
  action: string;
  actor: string | null;
  actor_id: number | null;
  detail: {
    setting?: string;
    group?: string;
    old?: string;
    new?: string;
    via?: string;
  } | null;
  created_at: string;
}

export interface SaveResult {
  group: string;
  saved: number;
  changed: string[];
  cache: string;
  effects: string[];
}

export interface ExportPayload {
  format: string;
  version: number;
  plaintext: boolean;
  count: number;
  masked_secrets: string[];
  settings: Record<string, string>;
}

export interface DiffRow {
  name: string;
  group: string;
  old: string;
  new: string;
}

export interface ImportDryRun {
  dry_run: true;
  unknown: string[];
  skipped_readonly: string[];
  will_change: number;
  diff: DiffRow[];
  effects: string[];
}

export interface ImportApplied {
  dry_run: false;
  unknown: string[];
  skipped_readonly: string[];
  applied: number;
  changed: string[];
  groups: string[];
  effects: string[];
}

// ============ 内容包（生态商店 M1） ============

/** GET /admin/content-packs/export?kind=taxonomy|theme 的包文件 */
export interface ContentPackFile {
  format: "fluxtorrent.contentpack";
  version: number;
  kind: "taxonomy" | "theme";
  pack_id: string;
  name: string;
  core_compat?: string;
  exported_at?: string;
  masked_secrets?: string[];
  payload: unknown;
}

/** 已装内容包行（GET /admin/content-packs） */
export interface ContentPackRow {
  id: number;
  pack_id: string;
  kind: "taxonomy" | "theme";
  name: string;
  version: string;
  applied_at: string;
}

/** POST /admin/content-packs/import（confirm=false / true 两态） */
export interface PackImportResult {
  dry_run: boolean;
  kind: "taxonomy" | "theme";
  pack_id: string;
  will_change?: number;
  current_categories?: number;
  pack_categories?: number;
  errors?: { name: string; message: string }[];
  applied?: { categories?: number; settings?: number };
  changed?: string[];
}

// ============ 商店目录（生态商店 M2） ============

/** GET /admin/content-packs/catalog 的条目（内置 ∪ 远程，带已装对照） */
export interface CatalogItem {
  pack_id: string;
  kind: "taxonomy" | "theme";
  name: string;
  description: string;
  version: string;
  source: "builtin" | "remote";
  site_type: string | null;
  url: string | null;
  installed_row_id?: number;
  installed_version?: string;
}

export interface CatalogResponse {
  items: CatalogItem[];
  count: number;
  remote_index: string | null;
  degraded: boolean;
}

// ============ 规则包（生态商店 M3） ============

/** POST /admin/content-packs/rule-try（试算，不落库） */
export interface RuleTryResult {
  key: string;
  expr: string;
  value: number | null;
  domain: [number, number];
  vars: string[];
}
