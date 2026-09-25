/**
 * 勋章管理·共享类型与常量（从 components/admin-medals.tsx 按域拆出）：
 * 勋章字典行 / 用户持有行类型与获取方式文案表。
 */

export interface MedalRow {
  id: number;
  name: string;
  description: string | null;
  price: number | null;
  rarity: string | null;
  limited: boolean;
  get_type: number;
  duration_days: number | null;
  bonus_addition_factor: number | null;
  category_id: number;
  asset_ref: string | null;
  /** per-勋章赠送手续费（基点；null = 回退全站 gift_tax_bp，0204） */
  gift_fee_bp: number | null;
  held_count: number;
}

export interface UserMedalRow {
  user_id: number;
  username: string;
  medal_id: number;
  medal_name: string;
  source: string;
  wearing: boolean;
  granted_at: string | null;
}

/** 获取方式取值键；显示名在 i18n `adminMedals.getType`（三语），
 *  由 `getTypeLabels()` 拼回原 `Record<number,string>` 形状 —— 消费方的
 *  `GET_TYPE[m.get_type] ?? m.get_type` 写法不用改。 */
export const GET_TYPE_KEYS = [1, 2, 3] as const;

export const getTypeLabels = (labels?: Record<string, string>) =>
  Object.fromEntries(
    GET_TYPE_KEYS.map((k) => [k, labels?.[String(k)] ?? String(k)]),
  ) as Record<number, string>;
