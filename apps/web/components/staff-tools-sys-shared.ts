/**
 * 系统观测·共享类型与常量（从 components/staff-tools-sys.tsx 按域拆出）：
 * 数据库状态 / 运行日志 / 位置管理 / 客户端规则行类型。
 */

export interface PgConn {
  state: string;
  count: number;
}

export interface TableSize {
  relname: string;
  total_size: number;
  row_estimates: number;
}

export interface DbStats {
  engine: string;
  database: string;
  connections: PgConn[];
  total_connections: number;
  database_size: number;
  slow_transactions: number;
  dead_tuples: number;
  tables: TableSize[];
}

export interface RuntimeLogItem {
  id: number;
  ts: string;
  level: string;
  source: string;
  target: string;
  message: string;
  repeat: number;
  instance: string;
}

export interface RuntimeLogPage {
  items: RuntimeLogItem[];
  total: number;
  page: number;
  per_page: number;
  pages: number;
  counts_24h: [string, number][];
  /** 近 7 天出现过的实例（0224 G30）：多副本部署时非空，前端据此显示筛选器 */
  instances: [string, number][];
}

export interface LocationItem {
  net: string;
  netmask: number;
  logins: number;
  users: number;
  failed: number;
  last_seen: string | null;
}

export interface LocationPage {
  items: LocationItem[];
  total: number;
  page: number;
  per_page: number;
  pages: number;
}

export interface AgentRule {
  id: number;
  mode: string;
  pattern: string;
  note: string | null;
  created_by: string | null;
  created_at: string;
}

/** 分页描边小按钮 */
export const PAGE_BTN_CLS =
  "min-h-[36px] rounded-full border border-line px-4 text-xs " +
  "font-bold disabled:opacity-40";
