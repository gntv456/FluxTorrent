/**
 * 内容域面板·共享类型（从 components/staff-tools-content.tsx 按域拆出）：
 * FAQ / 规则 / 分类 / 类型包行类型。
 */

export interface FaqItem {
  id: number;
  category: string;
  question: string;
  answer: string;
  sort: number;
}

export interface RuleItem {
  id: number;
  title: string;
  body: string;
  sort: number;
}

export interface CatItem {
  id: number;
  name: string;
  torrents: number;
  /** 图标键（0166）：film/tv/music/anime/game/app/book/sport/doc/edu；空=首字色块 */
  icon_key?: string;
  /** 分类色（0183）：#rrggbb；空=中性兜底 */
  bg_color?: string | null;
}

export interface TypePack {
  code: string;
  name: string;
  description: string | null;
  brand: string;
  categories: { id: number; name: string }[];
  modules: Record<string, boolean>;
  sort: number;
}
