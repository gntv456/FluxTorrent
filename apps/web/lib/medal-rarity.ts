/**
 * 勋章稀有度：**唯一真相源**（后台下拉与前台角标配色/中文标签都读这里）。
 *
 * 背景：`medals.rarity` 是自由文本，原先只有 `medals/page.tsx` 里两张硬编码 map 认识
 * legendary/epic/rare/common —— 后台表单于是只能让站长盲填英文枚举，填别的值就掉到
 * 「原样显示 + 蓝色兜底」。这里把词表抽出来共用，并保留自定义（站长可新增稀有度）。
 */
export const MEDAL_RARITIES = [
  { value: "legendary", label: "传说", style: "bg-sun text-ink" },
  { value: "epic", label: "史诗", style: "bg-indigo text-white" },
  { value: "rare", label: "稀有", style: "bg-sky text-white" },
  { value: "common", label: "普通", style: "bg-mint text-white" },
] as const;

/** 自定义 / 未知稀有度的兜底角标样式（不写死告警色，保持蓝色中性） */
export const MEDAL_RARITY_FALLBACK_STYLE = "bg-sky text-white";

export function medalRarityStyle(rarity?: string | null): string {
  return MEDAL_RARITIES.find((r) => r.value === rarity)?.style ?? MEDAL_RARITY_FALLBACK_STYLE;
}

/** 已知稀有度给中文名；自定义值原样返回（站长填什么就显示什么） */
export function medalRarityLabel(rarity?: string | null): string {
  if (!rarity) return "";
  return MEDAL_RARITIES.find((r) => r.value === rarity)?.label ?? rarity;
}

export function isKnownRarity(rarity?: string | null): boolean {
  return !!rarity && MEDAL_RARITIES.some((r) => r.value === rarity);
}
