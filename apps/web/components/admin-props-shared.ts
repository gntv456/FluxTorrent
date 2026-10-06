/**
 * 后台道具管理·共享类型与常量（从 components/admin-props.tsx 按域拆出）：
 * 商店道具/用户背包/头像框行类型、道具类型文案表、按类型的结构化
 * config 字段表，以及 config JSON ↔ 结构化字段互转工具。
 */

export interface ShopItemRow {
  id: number;
  name: string;
  kind: string;
  price: number;
  config: Record<string, unknown>;
  active: boolean;
  /** 库存配额（0287）：null=不限量 */
  stock_quota: number | null;
  stock_used: number;
}

export interface UserPropRow {
  order_id: number;
  user_id: number;
  username: string;
  item_id: number;
  item_name: string;
  kind: string;
  price: number;
  created_at: string;
  /** 是否已生效（0291 后端补下发）：回收只对「0 价 + 未生效」的单成立 */
  effect_applied: boolean;
}

export interface FrameRow {
  id: number;
  name: string;
  css: string;
  image_url: string | null;
  price: number;
  sort: number;
  worn_count: number;
}

/** 编辑态（新建/编辑道具表单） */
export interface EditState {
  id: number | null;
  f: {
    name: string;
    kind: string;
    price: string;
    config: string;
    active: boolean;
    stockQuota: string;
  };
}

/** 道具种类显示名；三语在 i18n `adminProps.kindLabel`，
 *  `CURRENCY` 占位符由消费方渲染时替换为货币名（本域既有约定）。 */
export const kindLabelMap = (
  labels?: Record<string, string>,
): Record<string, string> => labels ?? {};

/** 每种类型暴露的结构化 config 字段（自动拼装；装扮类的 slot/dressup 在 save 时自动补）。
 *  options: "frames" = 头像框库下拉；否则为 [值, 文案] 数组。wide = 占满一行。 */
interface KindField {
  key: string;
  label: string;
  num?: boolean;
  wide?: boolean;
  ph?: string;
  options?: [string, string][] | "frames";
}

/** 结构 spec（语言无关：字段键 / 数值 / 宽度 / 选项取值）。
 *  显示名与占位在 i18n `adminProps.fields`（键为 `kind.fieldKey`），
 *  选项文案在 `adminProps.opts`（键为选项值）。 */
type FieldSpec = {
  key: string;
  num?: boolean;
  wide?: boolean;
  options?: string[] | "frames";
};

const FIELD_SPEC: Record<string, FieldSpec[]> = {
  upload_credit: [{ key: "gb", num: true }],
  gift_spark: [{ key: "spark", num: true }],
  charity: [{ key: "spark", num: true }],
  custom_title: [{ key: "title", wide: true }],
  avatar_frame: [{ key: "avatar_url", options: "frames" }],
  animated_avatar: [{ key: "avatar_url", wide: true }],
  vip: [{ key: "days", num: true }],
  app_vip: [{ key: "days", num: true }],
  ad_free: [{ key: "days", num: true }],
  voucher_free: [{ key: "kind", options: ["free"] }],
  voucher_neutral: [{ key: "kind", options: ["neutral"] }],
  // invite / temp_invite / rename_card / makeup_card / rainbow_* 无额外字段
};

export const kindFields = (
  labels?: Record<string, { label: string; ph?: string }>,
  opts?: Record<string, string>,
): Record<string, KindField[]> => {
  const out: Record<string, KindField[]> = {};
  for (const [kind, fields] of Object.entries(FIELD_SPEC)) {
    out[kind] = fields.map((f) => {
      const m = labels?.[`${kind}.${f.key}`];
      return {
        key: f.key,
        label: m?.label ?? `${kind}.${f.key}`,
        num: f.num,
        wide: f.wide,
        ph: m?.ph,
        options:
          f.options === "frames"
            ? "frames"
            : f.options
              ? (f.options.map((v) => [v, opts?.[v] ?? v]) as [
                  string,
                  string,
                ][])
              : undefined,
      };
    });
  }
  return out;
};

/** config JSON ↔ 结构化字段互转（容错：解析失败返回空对象，不炸表单） */
export function cfgField(configJson: string): Record<string, string> {
  try {
    const o = JSON.parse(configJson || "{}");
    return Object.fromEntries(
      Object.entries(o).map(([k, v]) => [k, String(v ?? "")]),
    );
  } catch {
    return {};
  }
}

export function setCfgField(
  configJson: string,
  key: string,
  value: string,
): string {
  const o = cfgField(configJson);
  if (value === "") delete o[key];
  else o[key] = value;
  return JSON.stringify(o);
}
