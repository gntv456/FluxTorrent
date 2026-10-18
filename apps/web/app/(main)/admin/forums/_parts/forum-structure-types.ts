import type { ForumCategory } from "@fluxtorrent/domain-types";

/** 论坛结构管理：类型与共享常量。
 *  自 components/staff-tools-forums.tsx 迁入并扩展（sort / 分区计数）。 */

export interface ForumAdminForum {
  id: number;
  name: string;
  descr: string | null;
  /** 版块规则正文（0392）：null/空 = 未设置，前台不渲染规则条 */
  rules?: string | null;
  minclassread: number;
  minclasswrite: number;
  minclasscreate: number;
  protected: boolean;
  topics: number;
  /** 回复数（帖子总数；topics 表只存主题，帖子在 posts 表——与 Rust 行镜像） */
  posts: number;
  /** 分区内排序（0154） */
  sort: number;
  category_id?: number | null;
  category_name?: string | null;
  /** 乐观并发版本戳（0376）：编辑页回填 expected_mtime，保存时校验 */
  mtime?: string;
}

/** 版主元组 [forum_id, user_id, username]。
 *  后端此处保持元组返回（前端已按索引访问），显式标注避免再被误当对象。 */
export type ModTuple = [number, number, string];

/** 等级档（user_classes）：三档门槛下拉的候选 [id, name] 元组 */
export type ClassTuple = [number, string];

/** 版块抽屉的表单模型（新建/编辑共用；编辑=整行回填，防 B3 复发） */
export interface ForumBoardForm {
  name: string;
  descr: string;
  /** 版块规则正文（0392）：空串 = 清空（后端把缺省与空串分开处理） */
  rules: string;
  mr: number;
  mw: number;
  mc: number;
  prot: boolean;
  catId: number | "";
  sort: number;
}

/** 空表单（新建用；分区预选为当前选中的分区） */
export function blankBoardForm(defaultCatId: number | ""): ForumBoardForm {
  return {
    name: "",
    descr: "",
    rules: "",
    mr: 0,
    mw: 0,
    mc: 0,
    prot: false,
    catId: defaultCatId,
    sort: 0,
  };
}

/** 由已有版块构造表单 —— 这是「编辑不可用」的修复核心 */
export function formFromForum(f: ForumAdminForum): ForumBoardForm {
  return {
    name: f.name,
    descr: f.descr ?? "",
    rules: f.rules ?? "",
    mr: f.minclassread,
    mw: f.minclasswrite,
    mc: f.minclasscreate,
    prot: f.protected,
    catId: typeof f.category_id === "number" ? f.category_id : "",
    sort: f.sort,
  };
}

export interface ForumAdminData {
  forums: ForumAdminForum[];
  mods: ModTuple[];
  categories: ForumCategory[];
  classes?: ClassTuple[];
}

/** 左栏「未分组」伪条目 id（真实分区 id 均为正） */
export const NO_CATEGORY = -1;

/** 该版块归属的分区 id；null 归入「未分组」。
 *
 *  ⚠️ 不需要处理「指向已删分区」的悬空值：forums.category_id 外键是
 *  `ON DELETE SET NULL`，删分区时引用被置空 → 版块自动落到「未分组」。 */
export function catOf(f: ForumAdminForum): number {
  return typeof f.category_id === "number" ? f.category_id : NO_CATEGORY;
}

// ---- 共享样式（长 className 提常量，行宽门禁 ≤80） ----

export const INPUT_CLS =
  "min-h-[36px] w-full rounded-[var(--r-sm)] border " +
  "border-line bg-cloud px-3 text-sm outline-none focus:border-sky";

export const NUM_INPUT =
  "min-h-[36px] w-full rounded-[var(--r-sm)] border " +
  "border-line bg-cloud px-2 text-center text-sm outline-none " +
  "focus:border-sky";

export const BTN_SKY =
  "min-h-[36px] rounded-full bg-sky px-4 text-sm font-bold " +
  "text-white disabled:opacity-40";

export const BTN_LINE =
  "min-h-[36px] rounded-full border border-line px-3 text-sm text-sub";

export const BTN_ROW =
  "min-h-[26px] rounded-full px-2 text-xs font-bold text-sky " +
  "hover:bg-sky-soft";

export const BTN_ROW_DANGER =
  "min-h-[26px] rounded-full px-2 text-xs font-bold text-danger " +
  "hover:bg-danger-soft";

export const PANEL_CLS = "brand-panel p-4";

export const TAG_CLS =
  "inline-flex items-center gap-1 rounded-full border " +
  "border-line px-2 py-0.5 text-xs";
