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

export const GET_TYPE: Record<number, string> = {
  1: "兑换",
  2: "授予",
  3: "合成",
};
