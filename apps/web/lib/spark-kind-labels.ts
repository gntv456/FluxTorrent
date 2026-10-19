/**
 * 火花流水 kind → 人话（2026-09-30「字段名直出」全站收口批）。
 *
 * 键=后端 earn_spark/spend_spark 调用点的 kind 字面量全集
 * （apps/api 全仓 grep + 本库 spark_ledger distinct 双向核对）。
 * 同名簇归并：游戏四玩法/抽卡/娱乐屋统一「游戏娱乐」类，银行四流水
 * 统一「银行」类——用户看的是来源大类，不必区分内部动作细节。
 *
 * 使用：me/sparks 用户流水页与后台用户详情 SparkPanel 共用。
 *
 * ⚠️ 2026-10-20 接线更正：这里原先写成 `= zhCN.my.kinds`（直连源字典），
 * 于是 `getDict` 的段级回落机制对这张表完全不起作用——en/zh-TW 已经翻好的
 * 27 个 kind（shop / game / download / attendance / forum…全是流水里最常见的
 * 来源）在英文界面永远显示简体中文，属「翻了但没人读」。现在按调用方传进的
 * 当前字典取，回落仍由 i18n/server.ts 的深合并负责（未翻的键自动落 zh-CN）。
 */

import { zhCN, type Dict } from "@/i18n/zh-CN";

/** 当前语种的 kind 标签；不传 `dict.my` 时退回 zh-CN 源。 */
export function sparkKindLabels(
  my?: Pick<Dict["my"], "kinds">,
): Record<string, string> {
  const src = (my ?? zhCN.my).kinds;
  return (src ?? {}) as unknown as Record<string, string>;
}
