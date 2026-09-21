/**
 * 考核岗位·共享类型与工具（从 components/admin-exams.tsx 按域拆出）：
 * 岗位行类型、指标目录（跟随站点货币名动态化）、门槛行 ⇄ JSON
 * 双向转换（按 GB_KEYS 换算字节指标）。
 */

export interface JixiaoTypeRow {
  id: number;
  name: string;
  metrics: Record<string, unknown>;
  base_pay: number;
  min_requirements: Record<string, unknown>;
  bonus_rules: Record<string, unknown>;
  description?: string;
  /** 0106：本期登记人数（真实数据，旧版硬编码 "—"） */
  assigned_count?: number;
}

/** 指标目录：key 与后端 JIXIAO_METRIC_KEYS 白名单一一对应。
 *  货币名动态化：spark_delta 的标签/单位跟随站点 currency_name（默认「魔力」）。 */
export const metricOptions = (
  currency: string,
): { key: string; label: string; unit: string; hint: string }[] => [
  {
    key: "uploaded",
    label: "上传增量",
    unit: "GB",
    hint: "考核期内新增上传量",
  },
  {
    key: "downloaded",
    label: "下载增量",
    unit: "GB",
    hint: "考核期内新增下载量",
  },
  {
    key: "seed_hours",
    label: "做种时长",
    unit: "小时",
    hint: "考核期内累计做种小时数",
  },
  {
    key: "avg_seed_hours",
    label: "平均做种时间",
    unit: "小时/个",
    hint: "做种时长 ÷ 期内活跃种子数",
  },
  {
    key: "seed_days",
    label: "做种天数",
    unit: "天",
    hint: "考核期内有做种活动的天数",
  },
  {
    key: "spark_delta",
    label: `${currency}增量`,
    unit: currency,
    hint: `考核期内正向${currency}流水合计`,
  },
  {
    key: "seed_points_delta",
    label: "做种积分增量",
    unit: "积分",
    hint: "1 积分 = 1 小时做种",
  },
  {
    key: "uploads",
    label: "发种增量",
    unit: "个",
    hint: "考核期内新发布种子数",
  },
  {
    key: "seed_size_tb",
    label: "发布/做种体积",
    unit: "TB",
    hint: "当前在做种总体积",
  },
  {
    key: "seeding_count",
    label: "做种数量",
    unit: "个",
    hint: "当前在做种种子数",
  },
  {
    key: "ops",
    label: "审核/操作数量",
    unit: "次",
    hint: "考核期内管理操作数（audit_log）",
  },
];

/** 上传/下载指标在后端按字节存，表单用 GB 填写；seed_size_tb 本身就是 TB */
export const GB_KEYS = new Set(["uploaded", "downloaded"]);
export const GB = 1024 ** 3;

/** 一行门槛 = 指标 + 数值。列表编辑 ⇄ JSON 双向转换时按 GB_KEYS 换算。 */
export interface ReqRow {
  key: string;
  value: string;
}

export const EMPTY_FORM = {
  name: "",
  base_pay: "0",
  description: "",
  reqs: [{ key: "seed_hours", value: "100" }] as ReqRow[],
  bonusStep: "3",
  bonusPct: "10",
};

export function reqsToJson(reqs: ReqRow[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const r of reqs) {
    const v = Number(r.value);
    if (!r.key || !Number.isFinite(v) || v <= 0) continue;
    out[r.key] = GB_KEYS.has(r.key) ? Math.round(v * GB) : Math.round(v);
  }
  return out;
}

export function jsonToReqs(obj: Record<string, unknown>): ReqRow[] {
  const rows = Object.entries(obj)
    .filter(([, v]) => typeof v === "number" && v > 0)
    .map(([k, v]) => ({
      key: k,
      value: String(
        GB_KEYS.has(k) ? Math.round((v as number) / GB) : (v as number),
      ),
    }));
  return rows.length ? rows : [{ key: "seed_hours", value: "100" }];
}
