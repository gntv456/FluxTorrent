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
  };
}

export const KIND_LABEL: Record<string, string> = {
  upload_credit: "上传量",
  invite: "邀请",
  temp_invite: "临时邀请",
  gift_spark: "CURRENCY",
  custom_title: "头衔卡",
  rename_card: "改名卡",
  makeup_card: "补签卡",
  rainbow_name: "彩虹名",
  rainbow_id: "彩虹ID",
  avatar_frame: "头像框",
  animated_avatar: "动态头像",
  vip: "VIP",
  app_vip: "APP VIP",
  ad_free: "去广告",
  charity: "公益",
};

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

export const KIND_FIELDS: Record<string, KindField[]> = {
  upload_credit: [{ key: "gb", label: "上传量(GB)", num: true, ph: "10" }],
  gift_spark: [{ key: "spark", label: `CURRENCY 数量`, num: true, ph: "5000" }],
  charity: [
    { key: "spark", label: "捐赠 CURRENCY 数（空=按价格全额）", num: true },
  ],
  custom_title: [
    { key: "title", label: "头衔文字", wide: true, ph: "种田大户" },
  ],
  avatar_frame: [
    { key: "avatar_url", label: "关联头像框（佩戴时生效）", options: "frames" },
  ],
  animated_avatar: [
    {
      key: "avatar_url",
      label: "头像图片 URL（GIF 动图）",
      wide: true,
      ph: "https://…/cat.gif",
    },
  ],
  vip: [{ key: "days", label: "时长(天)", num: true, ph: "30" }],
  app_vip: [{ key: "days", label: "时长(天)", num: true, ph: "30" }],
  ad_free: [{ key: "days", label: "时长(天)", num: true, ph: "15" }],
  voucher_free: [{ key: "kind", label: "券种", options: [["free", "免费券"]] }],
  voucher_neutral: [
    { key: "kind", label: "券种", options: [["neutral", "中性券"]] },
  ],
  // invite / temp_invite / rename_card / makeup_card / rainbow_* 无额外字段
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
