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
 *  显示名/单位/说明在 i18n `adminExams.metrics`（三语）；`{magic}` 占位符
 *  在此处替换为站点货币名（与全站 fmtCur 约定一致）。
 *  `labels` 缺省时回落指标 key 本身（不会渲染成空白）。 */
export const METRIC_KEYS = [
  "uploaded",
  "downloaded",
  "seed_hours",
  "avg_seed_hours",
  "seed_days",
  "spark_delta",
  "seed_points_delta",
  "uploads",
  "seed_size",
  "seed_size_tb",
  "seeding_count",
  "ops",
] as const;

export interface MetricMeta {
  key: string;
  label: string;
  unit: string;
  hint: string;
}

export const metricOptions = (
  currency: string,
  labels?: Record<string, { label: string; unit: string; hint: string }>,
): MetricMeta[] =>
  METRIC_KEYS.map((key) => {
    const m = labels?.[key];
    const sub = (s: string) => s.replace(/\{magic\}/g, currency);
    return {
      key,
      label: sub(m?.label ?? key),
      unit: sub(m?.unit ?? ""),
      hint: sub(m?.hint ?? ""),
    };
  });

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
