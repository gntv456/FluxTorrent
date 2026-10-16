/**
 * 火花流水 kind → 人话（2026-09-30「字段名直出」全站收口批）。
 *
 * 键=后端 earn_spark/spend_spark 调用点的 kind 字面量全集
 * （apps/api 全仓 grep + 本库 spark_ledger distinct 双向核对）。
 * 同名簇归并：游戏四玩法/抽卡/娱乐屋统一「游戏娱乐」类，银行四流水
 * 统一「银行」类——用户看的是来源大类，不必区分内部动作细节。
 *
 * 使用：me/sparks 用户流水页与后台用户详情 SparkPanel 共用。
 * i18n 收口批：文案改由 zh-CN 源字典 me.kinds 提供（en/zh-TW/ja 随
 * getDict 深合并各自覆盖），本文件只保留键序与回落逻辑；未收录 kind
 * 回落原始 key，不静默。
 */

import { zhCN } from "@/i18n/zh-CN";

export const SPARK_KIND_LABELS: Record<string, string> =
  zhCN.my.kinds as Record<string, string>;
