/** 批量发放面板的 kind 选项表（0286 从 increment-bulk.tsx 拆出守 300 行门禁）。 */

import type { Dict } from "@/i18n/zh-CN";

type BulkDict = Dict["adminBulk"];

export function kindsOf(
  currency: string,
  t: BulkDict,
): [string, string, string][] {
  return [
    ["spark", currency, t.kindSpark],
    ["uploaded", t.labelUploaded, t.kindUploaded],
    ["invite", t.labelInvite, t.kindInvite],
    ["resub_card", t.labelResub, t.kindResub],
    ["medal", t.labelMedal, t.kindMedal],
    ["item", t.labelItem, t.kindItem],
  ];
}
