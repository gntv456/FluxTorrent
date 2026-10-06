/** 批量发放的类型与幂等键（0291 从 increment-bulk.tsx 拆出守 300 行门禁）。 */

import { idemKey } from "@/lib/idem-key";

export interface RoleDef {
  key: string;
  name: string;
}

export interface MedalDef {
  id: number;
  name: string;
}

export interface ItemDef {
  id: number;
  name: string;
  kind: string;
}

export interface BulkResult {
  affected: number;
  targets: number;
  kind: string;
  amount: number;
  batch_id?: string;
  target_ids?: number[];
  /** 0291 试运行库存体检：false = 这一批发不出去 */
  stock_ok?: boolean;
  /** 0291 台账回写状态：不是 "ok" 时这批发放的留档没落全 */
  ledger?: string;
}

/** 批量发放表单的当前快照（payload 组装从组件里搬出来，
 *  组件只留视图；键计算与 payload 形状必须同源，否则键与内容会漂移）。 */
export interface BulkFormState {
  kind: string;
  amount: string;
  classes: number[];
  roles: string[];
  userIds: string;
  days: string;
  medalId: string;
  itemId: string;
  subject: string;
  body: string;
  sender: string;
  email: boolean;
}

/** 组装请求体：字段口径与后端 IncrementBulkReq 一一对应。 */
export function buildBulkPayload(
  s: BulkFormState,
): Record<string, unknown> {
  const payload: Record<string, unknown> = {
    kind: s.kind,
    amount: Number(s.amount),
    classes: [...s.classes],
    roles: s.roles,
    user_ids: s.userIds
      .split(/[,，\s]+/)
      .map(Number)
      .filter((n) => n > 0),
  };
  if (s.days.trim()) payload.days = Number(s.days);
  if (s.kind === "medal" && s.medalId) payload.medal_id = Number(s.medalId);
  if (s.kind === "item" && s.itemId) payload.item_id = Number(s.itemId);
  if (s.subject.trim()) payload.subject = s.subject.trim();
  if (s.body.trim()) payload.body = s.body.trim();
  payload.sender = s.sender;
  if (s.email) payload.email = true;
  return payload;
}

/** FNV-1a 与按天分桶都在 `@/lib/idem-key`（全站一份，别在这儿再写一把尺）。 */

/** 发放幂等键：同一份表单内容在同一天里算同一个键，口径见 lib/idem-key。 */
export function bulkIdemKey(payload: unknown): string {
  return idemKey(payload, "bulk");
}
