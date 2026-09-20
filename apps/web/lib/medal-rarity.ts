/**
 * 勋章稀有度：**词表在库里**（`medal_rarities` 表 + 0143 迁移），后台可增删改名，前台读同一份。
 *
 * 这里只放两样东西：
 * 1. `MEDAL_RARITY_TONES` —— 允许的配色档（固定 6 档 → tailwind token 类）。
 *    库里只存档位标识，不存 CSS，避免站长写任意样式类把主题体系打破。
 * 2. 接口失败时的内置兜底词表（= 改造前前端硬编码的那 4 个值），保证页面不空。
 */
export interface MedalRarity {
  value: string;
  label: string;
  tone: string;
  sort?: number;
  /** 后台列表用：当前有多少枚勋章在用 */
  used?: number;
}

/** 配色档（值存库；改这里前先确认前端已生成对应 tailwind 类） */
export const MEDAL_RARITY_TONES = [
  { value: "gold", label: "金（传说）", style: "bg-sun text-ink" },
  { value: "coral", label: "橙（热卖）", style: "bg-coral text-white" },
  { value: "mint", label: "绿（普通）", style: "bg-mint text-white" },
  { value: "sky", label: "蓝（信息）", style: "bg-sky text-white" },
  { value: "indigo", label: "靛（史诗）", style: "bg-indigo text-white" },
  { value: "candy", label: "粉（限定）", style: "bg-candy text-white" },
] as const;

/** 接口拿不到词表时的兜底（与 0143 的种子数据一致） */
export const DEFAULT_MEDAL_RARITIES: MedalRarity[] = [
  { value: "legendary", label: "传说", tone: "gold", sort: 10 },
  { value: "epic", label: "史诗", tone: "indigo", sort: 20 },
  { value: "rare", label: "稀有", tone: "sky", sort: 30 },
  { value: "common", label: "普通", tone: "mint", sort: 40 },
];

/** 词表里没有的稀有度（历史数据 / 站长手改过库）→ 中性蓝，不崩样式 */
export const MEDAL_RARITY_FALLBACK_STYLE = "bg-sky text-white";

export function rarityToneStyle(tone?: string | null): string {
  return MEDAL_RARITY_TONES.find((t) => t.value === tone)?.style ?? MEDAL_RARITY_FALLBACK_STYLE;
}

export function medalRarity(list: MedalRarity[], value?: string | null): MedalRarity | undefined {
  return value ? list.find((r) => r.value === value) : undefined;
}

export function medalRarityStyle(list: MedalRarity[], value?: string | null): string {
  return rarityToneStyle(medalRarity(list, value)?.tone);
}

/** 词表收录 → 显示名；未收录 → 原样显示键（不隐藏数据，便于站长发现漏配） */
export function medalRarityLabel(list: MedalRarity[], value?: string | null): string {
  if (!value) return "";
  return medalRarity(list, value)?.label ?? value;
}

export function isKnownRarity(list: MedalRarity[], value?: string | null): boolean {
  return !!medalRarity(list, value);
}

/** slug 归一化（与后端 clean_rarity_value 同口径，用于前端即时校验/预览） */
export function cleanRarityValue(raw: string): string {
  return raw
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]/g, "")
    .slice(0, 32);
}
