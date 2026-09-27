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
  /** 父分类（0188 层级）：NULL = 顶级 */
  parent_id?: number | null;
  /** 排序值（0195）：小的在前，同值回落 id */
  sort: number;
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

/** 站型包应用记录（G21）：回看 diff/计数，回滚 = 重放应用前快照 */
export interface PackApplyItem {
  id: number;
  pack_code: string;
  pack_name: string;
  mode: string;
  changes: { key: string; old: string; new: string }[];
  counts: Record<string, unknown>;
  applied_at: string;
  actor: string | null;
  rolled_back_at: string | null;
}
