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
