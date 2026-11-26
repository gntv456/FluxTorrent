/**
 * 命名向导（0525，对照 StageAbove 发布页「填写信息」自动拼标题）：
 * 从质量维度选中值（secVals/secDict）+ 元数据建议名拼装规范主标题。
 * 纯函数，无副作用——单测/预览/应用三处共用同一拼装口径。
 *
 * 拼装格式（NexusPHP 惯例）：
 *   中文名 English.Title 2026 WEB-DL 1080p H.265-GROUP
 * 段间单空格，制作组用连字符挂在末尾；空段自动跳过。
 */

/** 发布表单的维度字典行（与 /section-dict 响应同形） */
export interface SecDictRow {
  id: number;
  kind: string;
  name: string;
  sort: number;
}

/** 维度定义（field_type 决定 secVals 里值的形态） */
export interface SecKindMeta {
  kind: string;
  label: string;
  field_type: string;
  multiple: boolean;
}

/**
 * 从表单状态解析某维度的展示名（兼容三种形态）：
 * - select：secVals 存 dict_id 字符串 → 反查字典 name
 * - text/number：存原文 → 直接用
 * - multiselect：存逗号串 → 反查多个 name 用 + 连接（NP 惯例）
 * 找不到字典行或值为空返回空串。
 */
export function resolveSecName(
  kind: string,
  secVals: Record<string, string>,
  secDict: Record<string, SecDictRow[]>,
): string {
  const raw = (secVals[kind] ?? "").trim();
  if (!raw) return "";
  const rows = secDict[kind] ?? [];
  if (rows.length === 0) return raw; // text/number 自由值：字典为空即原文
  const parts = raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((idStr) => rows.find((r) => String(r.id) === idStr)?.name ?? "")
    .filter(Boolean);
  if (parts.length === 0) return raw; // dict_id 不在字典里（换分类残留）：保原文
  return parts.join("+");
}

/**
 * 拼装主标题。各段取值：
 * - cn/en/year：来自 PT-Gen 元数据建议（metaName 已含「中文名 外文名 (年份)」时
 *   直接作为前缀段），缺失则跳过
 * - source/standard/codec/audio/team：质量维度展示名
 * 非空段以单空格连接；team 段用连字符与前段衔接（NP 惯例 -GROUP）。
 */
export function buildName(parts: {
  metaName?: string;
  source?: string;
  standard?: string;
  codec?: string;
  audio?: string;
  team?: string;
}): string {
  const segs: string[] = [];
  const meta = (parts.metaName ?? "").trim();
  if (meta) segs.push(meta);
  for (const k of ["source", "standard", "codec", "audio"] as const) {
    const v = (parts[k] ?? "").trim();
    if (v) segs.push(v);
  }
  let out = segs.join(" ");
  const team = (parts.team ?? "").trim();
  if (team) {
    // team 段连字符衔接；无前段时独立成段
    out = out ? `${out}-${team}` : team;
  }
  return out;
}

/**
 * 从 PT-Gen 建议名提取年份（用于预览校验，缺省空）。
 * 「荒年娶妻 2026」/「Title (2024)」→ "2026" / "2024"。
 */
export function extractYear(metaName: string): string {
  const m = metaName.match(/(19|20)\d{2}/);
  return m ? m[0] : "";
}
