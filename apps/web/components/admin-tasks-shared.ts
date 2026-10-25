/**
 * 任务定义·共享类型与常量（从 components/admin-tasks.tsx 按域拆出）：
 * 任务行类型、空白表单值、任务种类/周期字典与时间本地化工具。
 */

export interface TaskRow {
  id: number;
  name: string;
  metric: Record<string, unknown>;
  starts_at: string;
  ends_at: string;
  target_class: number;
  reward: number;
  penalty: number;
  claim_limit: number | null;
  kind: string;
  auto_assign: boolean;
  period: string;
  /** 以下为 0093 考核引擎字段（本次补齐为可配） */
  duration_days: number;
  subtitle: string | null;
  tier: string | null;
  fee: number;
  quota_total: number;
  sort: number;
  /** 0397 考核不达标处置菜单（多选+参数化；普通任务不消费） */
  fail_penalty?: PenaltyItem[];
}

/** 0397 处置菜单项（与后端 PENALTY_KINDS 同一份菜单） */
export interface PenaltyItem {
  kind: string;
  amount?: number;
  target?: string;
  pages?: string[];
}

/** 处置菜单键（worker penalty.rs / api tasks.rs 三处镜像） */
export const PENALTY_KEYS = [
  "ban",
  "suspend",
  "restrict_download",
  "mute",
  "ban_upload",
  "ban_pages",
  "spark_fine",
  "traffic_fine",
] as const;

/** 需要金额参数的处置项（表单为其渲染数值输入） */
export const PENALTY_AMOUNT_KEYS = new Set(["spark_fine", "traffic_fine"]);
/** 需要页面列表参数的处置项 */
export const PENALTY_PAGES_KEYS = new Set(["ban_pages"]);
/** 需要目标量参数（uploaded/downloaded）的处置项 */
export const PENALTY_TARGET_KEYS = new Set(["traffic_fine"]);

export const EMPTY = {
  name: "",
  metric: "{}",
  starts_at: "",
  ends_at: "",
  target_class: "0",
  reward: "0",
  penalty: "0",
  claim_limit: "",
  kind: "task",
  auto_assign: false,
  period: "once",
  duration_days: "30",
  subtitle: "",
  tier: "",
  fee: "0",
  quota_total: "200",
  sort: "0",
  /** 0397 处置菜单：选中项集合（勾选状态由 form 持有） */
  penaltyChecked: {} as Record<string, boolean>,
  /** 0397 处置菜单：选中项的参数（金额/目标量/页面列表） */
  penaltyParams: {} as Record<
    string,
    { amount?: string; target?: string; pages?: string }
  >,
};

/** 任务种类 / 周期的取值键。显示名在 i18n `adminTasks.kinds` / `.periods`
 *  （三语），由 `kindList()` / `periodList()` 拼回原 `{v, label}` 形状 ——
 *  消费方的 `.find((k) => k.v === ...)` 逻辑因此不用改。 */
export const KIND_KEYS = ["task", "onboard", "periodic"] as const;
export const PERIOD_KEYS = ["once", "monthly", "quarterly"] as const;

/** fail_penalty 行（表单勾选集 + 参数）→ 提交数组的双向转换 */
export function toPenaltyItems(
  checked: Record<string, boolean>,
  params: Record<string, { amount?: string; target?: string; pages?: string }>,
): PenaltyItem[] {
  const out: PenaltyItem[] = [];
  for (const k of PENALTY_KEYS) {
    if (!checked[k]) continue;
    const p = params[k] ?? {};
    const item: PenaltyItem = { kind: k };
    if (PENALTY_AMOUNT_KEYS.has(k)) {
      const amt = Math.max(0, Number(p.amount) || 0);
      // 数额型处置缺数额＝「配了但不扣」：后端会拒（数额需大于 0），
      // 这里直接不提交，免得站长填完表单被一句 400 打回。
      if (amt <= 0) continue;
      item.amount = amt;
    }
    if (PENALTY_TARGET_KEYS.has(k)) {
      item.target = p.target === "downloaded" ? "downloaded" : "uploaded";
    }
    if (PENALTY_PAGES_KEYS.has(k)) {
      const pages = (p.pages ?? "")
        .split(/[,，\s]+/)
        .map((s) => s.trim())
        .filter(Boolean);
      // 没填权限位＝空菜单项，后端会拒（至少一个权限位）
      if (pages.length === 0) continue;
      item.pages = pages;
    }
    out.push(item);
  }
  return out;
}

/** 后端 fail_penalty 数组 → 表单勾选集 + 参数（编辑回显用） */
export function fromPenaltyItems(items: PenaltyItem[] | undefined | null): {
  checked: Record<string, boolean>;
  params: Record<string, { amount?: string; target?: string; pages?: string }>;
} {
  const checked: Record<string, boolean> = {};
  const params: Record<
    string,
    {
      amount?: string;
      target?: string;
      pages?: string;
    }
  > = {};
  for (const it of items ?? []) {
    if (!PENALTY_KEYS.includes(it.kind as (typeof PENALTY_KEYS)[number])) {
      continue;
    }
    checked[it.kind] = true;
    params[it.kind] = {
      amount: it.amount != null ? String(it.amount) : "",
      target: it.target ?? "uploaded",
      pages: (it.pages ?? []).join(", "),
    };
  }
  return { checked, params };
}

type Labelled<V extends string> = { v: V; label: string };
const build = <V extends string>(
  keys: readonly V[],
  labels: Record<string, string> | undefined,
): Labelled<V>[] => keys.map((v) => ({ v, label: labels?.[v] ?? v }));

export const kindList = (labels?: Record<string, string>) =>
  build(KIND_KEYS, labels);
export const periodList = (labels?: Record<string, string>) =>
  build(PERIOD_KEYS, labels);

export function toLocalInput(iso: string): string {
  const d = new Date(iso);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000)
    .toISOString()
    .slice(0, 16);
}
