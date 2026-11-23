/**
 * 老虎机 / 水果机符号表（纯哑件，无 hooks）。
 *
 * 后端池档位 label 是中文串（「柠檬」「777 大奖」「谢谢参与」…），这里按
 * 关键词把档位映射到街机符号；匹配不到时按环序回落，保证滚轮永远有格可画。
 * 符号两套共用：老虎机偏经典 777/BAR/钻石，水果机偏参考图水果列。
 */

export interface SlotSymbol {
  /** 稳定键：滚轮环与高亮都按它比对 */
  id: string;
  /** 大字图案（emoji，零资源依赖） */
  icon: string;
  /** 街机字形（BAR / 777 这类文字牌），无则空 */
  glyph?: string;
  /** 牌面底色令牌（CSS 变量或 hex） */
  tone: string;
  /** 关键词：命中池档位 label 的任一关键词即归此符号 */
  keys: string[];
}

/** 经典老虎机符号环（三滚轮停格候选） */
export const SLOT_REEL: SlotSymbol[] = [
  {
    id: "seven",
    icon: "🎰",
    glyph: "777",
    tone: "#c0272d",
    keys: ["777", "大奖", "传奇"],
  },
  { id: "bar", icon: "🧮", glyph: "BAR", tone: "#3a2a1a", keys: ["BAR"] },
  { id: "bell", icon: "🔔", tone: "#e9b94e", keys: ["铃铛", "铃"] },
  { id: "diamond", icon: "💎", tone: "#3fa7d6", keys: ["钻石", "宝石"] },
  { id: "star", icon: "⭐", tone: "#e8c33a", keys: ["星"] },
  { id: "cherry", icon: "🍒", tone: "#d8445c", keys: ["樱桃"] },
  { id: "lemon", icon: "🍋", tone: "#cfe24a", keys: ["柠"] },
  { id: "orange", icon: "🍊", tone: "#f28c28", keys: ["橙"] },
];

/** 水果机符号环（参考图：BAR / 77 / 星星 / 西瓜 / 铃铛 / 青柠 / 橙子 / 苹果） */
export const FRUIT_REEL: SlotSymbol[] = [
  { id: "bar", icon: "🧮", glyph: "BAR", tone: "#3a2a1a", keys: ["BAR"] },
  {
    id: "seven",
    icon: "🎰",
    glyph: "77",
    tone: "#c0272d",
    keys: ["777", "77", "大奖", "传奇"],
  },
  { id: "star", icon: "⭐", tone: "#e8c33a", keys: ["星"] },
  { id: "melon", icon: "🍉", tone: "#3fae5a", keys: ["西瓜"] },
  { id: "bell", icon: "🔔", tone: "#e9b94e", keys: ["铃铛", "铃"] },
  { id: "lime", icon: "🍈", tone: "#7cc24a", keys: ["青柠", "柠檬", "柠"] },
  { id: "orange", icon: "🍊", tone: "#f28c28", keys: ["橙"] },
  { id: "apple", icon: "🍎", tone: "#d8445c", keys: ["苹果", "红苹果"] },
];

/** 谢谢参与 / 未中奖的回落格 */
export const BLANK_SYMBOL: SlotSymbol = {
  id: "blank",
  icon: "🍀",
  tone: "#5a6478",
  keys: ["谢谢", "未中", "空"],
};

/** 按池档位 label 取符号；环序回落，未知档落到末位装饰格。 */
export function symbolForLabel(
  reel: SlotSymbol[],
  label: string | undefined,
  index: number,
): SlotSymbol {
  const text = label ?? "";
  for (const s of reel) {
    if (s.keys.some((k) => text.includes(k))) return s;
  }
  if (/谢谢|未中|空/.test(text)) return BLANK_SYMBOL;
  return reel[index % reel.length] ?? BLANK_SYMBOL;
}

/** 滚轮空闲时的默认三格（环序取，保证首屏不是空窗） */
export function idleReel(reel: SlotSymbol[], cols = 3): SlotSymbol[] {
  return Array.from({ length: cols }, (_, i) => reel[i % reel.length]);
}
