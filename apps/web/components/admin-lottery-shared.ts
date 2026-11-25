/** 固定抽签运营面板·共享元数据（0523 人性化批）。
 *  奖励类型清单 + 每类配置字段的输入规格（标签键/类型/占位/缺省）+
 *  状态/触发方式 → 字典键映射。表单（admin-lottery-create）与
 *  列表（admin-fixed-lottery）共用这一把尺；字段口径与后端
 *  fixed_lottery_admin_write.rs::validate_reward_config 一一对应。 */

import type { useI18n } from "@/i18n/client";

type Dict = ReturnType<typeof useI18n>["dict"];
export type LotteryDict = Dict["adminLottery"];

/** 配置字段输入规格：kind=text 走文本框，kind=int 走数值框。 */
export type CfgField = {
  key: string;
  kind: "text" | "int";
  /** 字典键：t[cfgLabel[key]] 取标签 */
  labelKey: keyof LotteryDict;
  min?: number;
  defaultValue: number | string;
  /** 后端缺省口径备注（title 属性提示） */
  hintKey?: keyof LotteryDict;
};

/** 奖励类型元数据：字典键 labelKey 取显示名；fields 为结构化
 *  配置字段（与后端校验/发放侧真实消费的键一致）。 */
export const LOTTERY_KINDS: {
  kind: string;
  labelKey: keyof LotteryDict;
  fields: CfgField[];
}[] = [
  { kind: "spark", labelKey: "kSpark", fields: [
    { key: "amount", kind: "int", labelKey: "cAmount", min: 1,
      defaultValue: 100 },
  ] },
  { kind: "invite", labelKey: "kInvite", fields: [] },
  { kind: "item", labelKey: "kItem", fields: [
    { key: "item_key", kind: "text", labelKey: "cItemKey",
      defaultValue: "" },
  ] },
  { kind: "medal", labelKey: "kMedal", fields: [
    { key: "medal_id", kind: "int", labelKey: "cMedalId", min: 1,
      defaultValue: 1 },
    { key: "days", kind: "int", labelKey: "cDays", min: 0,
      defaultValue: 0, hintKey: "cDaysHint" },
  ] },
  { kind: "upload", labelKey: "kUpload", fields: [
    { key: "gb", kind: "int", labelKey: "cGb", min: 1, defaultValue: 10 },
  ] },
  { kind: "vip", labelKey: "kVip", fields: [
    { key: "days", kind: "int", labelKey: "cDays", min: 1,
      defaultValue: 7, hintKey: "cDaysHint" },
  ] },
  { kind: "ad_free", labelKey: "kAdFree", fields: [
    { key: "days", kind: "int", labelKey: "cDays", min: 1,
      defaultValue: 15, hintKey: "cDaysHint" },
  ] },
  { kind: "rainbow_id", labelKey: "kRainbow", fields: [
    { key: "item_id", kind: "int", labelKey: "cItemId", min: 1,
      defaultValue: 1 },
  ] },
  { kind: "rename_card", labelKey: "kRename", fields: [] },
  { kind: "custom", labelKey: "kCustom", fields: [] },
];

export const kindMeta = (kind: string) =>
  LOTTERY_KINDS.find((k) => k.kind === kind) ?? LOTTERY_KINDS[0];

/** 按字段规格把结构化值序列化成后端 reward_config。 */
export function buildConfig(
  fields: CfgField[],
  values: Record<string, number | string>,
): Record<string, unknown> {
  const cfg: Record<string, unknown> = {};
  for (const f of fields) {
    const v = values[f.key];
    if (f.kind === "int") {
      const n = Number(v);
      if (Number.isFinite(n) && n > 0) cfg[f.key] = Math.trunc(n);
    } else if (typeof v === "string" && v.trim()) {
      cfg[f.key] = v.trim();
    }
  }
  return cfg;
}

/** 活动状态 → 字典键（列表列显示本地语言而非裸枚举）。 */
export const STATUS_KEYS: Record<string, keyof LotteryDict> = {
  draft: "stDraft",
  open: "stOpen",
  drawn: "stDrawn",
  cancelled: "stCancelled",
};

/** 触发方式 → 字典键。 */
export const TRIGGER_KEYS: Record<string, keyof LotteryDict> = {
  scheduled: "trigScheduled",
  threshold: "trigThreshold",
};

/** 中奖状态 → 字典键。 */
export const WINNER_KEYS: Record<string, keyof LotteryDict> = {
  pending: "wPending",
  fulfilled: "wFulfilled",
  satisfied: "wSatisfied",
  failed: "wFailed",
  claimed: "wClaimed",
};

/** 履约状态 → 字典键。 */
export const FULFILL_KEYS: Record<string, keyof LotteryDict> = {
  pending: "wPending",
  fulfilled: "wFulfilled",
  satisfied: "wSatisfied",
  failed: "wFailed",
  exhausted: "wExhausted",
};

/** 从字典按键名取串（statusKey 等映射用；键缺失回退原文）。 */
export function dictLabel(
  t: LotteryDict,
  keys: Record<string, keyof LotteryDict>,
  raw: string,
): string {
  const k = keys[raw];
  return k ? String(t[k]) : raw;
}
