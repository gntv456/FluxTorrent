/** 分类图标集 · 主题映射（V3/V4 位图图标接线层，本轮新增、不改既有文件）。
 *
 * 背景：两套已定稿的分类图标均为 256×256 位图（PNG 内嵌 SVG 封装），
 * 非 currentColor 矢量，与 components/icons.tsx 的单色 PATHS 口径不同，
 * 因此走独立目录 + 独立组件（components/cat-icon.tsx），不进 PATHS。
 *
 * 资产落位（下一步执行，本文件仅声明约定）：
 *   design/icons/v3-shanhaijing/v3.1-props/png → apps/web/public/cat-icons/shanhaijing/
 *   design/icons/v4-aurora/png                 → apps/web/public/cat-icons/aurora/
 *
 * 文件名口径：cat-<语义名>-<64|128|256|512>.png（源目录有版本号，落位后
 * 目录名用主题语义 shanhaijing / aurora，避免版本号进了生产 URL）。
 *
 * 版本说明：shanhaijing 集 2026-11-03 起指向 v3.1-props（道具章纹重绘版，
 * 22px 可辨；旧 v3 插画版细节过密，16/22px 不达标，留档不再接入）。
 */

import type { ThemeId } from "@/lib/themes";

export type CatIconSetId = "shanhaijing" | "aurora";

export interface CatIconSet {
  /** public/cat-icons/ 下的目录名 */
  dir: CatIconSetId;
  /** 显示名 i18n 键（dict.common 下；文案不进本模块，i18n 门禁口径） */
  labelKey: string;
  /** 说明文案 i18n 键（同上） */
  noteKey: string;
}

export const CAT_ICON_SETS: Record<CatIconSetId, CatIconSet> = {
  shanhaijing: {
    dir: "shanhaijing",
    labelKey: "catIconSetShanhaijing",
    noteKey: "catIconSetShanhaijingNote",
  },
  aurora: {
    dir: "aurora",
    labelKey: "catIconSetAurora",
    noteKey: "catIconSetAuroraNote",
  },
};

/** 主题 → 图标集：山海经用山海经套；品牌亮/暗统一 aurora 套。
 *  判定口径（2026-11-03 主题线确认）：
 *  ① 现有 id → 图标集：brand / brand-night → aurora；shanhaijing →
 *     v3.1 道具章纹套（CatIconSetId "shanhaijing"）。
 *  ② tide 不是独立主题 id——它是 brand 亮色的皮（样式挂在
 *     [data-theme="brand"] 作用域），自然归 aurora，不为它写显式映射。
 *  ③ 未注册主题一律兜底 aurora；未来若真新增独立主题 id，需在此处
 *     补一行显式映射并同步更新相关测试。 */
export const THEME_TO_CAT_ICON_SET: Record<ThemeId, CatIconSetId> = {
  brand: "aurora",
  "brand-night": "aurora",
  shanhaijing: "shanhaijing",
};

/** 未知/未来主题 id 的兜底集（含 tide 待确认项） */
export const DEFAULT_CAT_ICON_SET: CatIconSetId = "aurora";

export function catIconSetForTheme(theme: string): CatIconSetId {
  return (THEME_TO_CAT_ICON_SET as Record<string, CatIconSetId>)[theme] ??
    DEFAULT_CAT_ICON_SET;
}

/** 分类 key（categories.key，库内稳定标识）→ 图标文件名（不含扩展名）。
 *  两套图标集同构（同名文件），一张表共用。 */
export const CAT_KEY_TO_FILE: Record<string, string> = {
  general_movie: "cat-movie",
  general_tv: "cat-tv",
  general_shortdrama: "cat-shortdrama",
  general_variety: "cat-variety",
  general_doc: "cat-documentary",
  general_anime: "cat-anime",
  general_music: "cat-music",
  general_audiobook: "cat-audiobook",
  general_game: "cat-game",
  general_software: "cat-software",
  general_ebook: "cat-ebook",
  general_edu: "cat-edu",
  general_sports: "cat-sports",
  // 扩展包同语义分类（站型切换后仍命中）
  ebook_comic: "cat-comic",
  anime_manga: "cat-comic",
  ebook_audio: "cat-audiobook",
  movie_tv: "cat-tv",
  movie_variety: "cat-variety",
  movie_doc: "cat-documentary",
  music_mv: "cat-tv",
  sport_football: "cat-sports",
};

/** icon_key（categories.icon_key，站长后台可填的旧词表）→ 文件名。
 *  当分类没有 key（或未下发）时的次级解析路径。 */
export const ICON_KEY_TO_FILE: Record<string, string> = {
  film: "cat-movie",
  tv: "cat-tv",
  music: "cat-music",
  anime: "cat-anime",
  app: "cat-software",
  book: "cat-ebook",
  sport: "cat-sports",
  doc: "cat-documentary",
  edu: "cat-edu",
  game: "cat-game",
  file: "cat-other",
  // 0458 解撞后的专属键（与旧键并存，见 tests/category-icon-dedup.test.tsx）
  shortdrama: "cat-shortdrama",
  audiobook: "cat-audiobook",
  variety: "cat-variety",
};

/** 通用兜底（两套均有 cat-default.svg / cat-other.svg） */
export const FALLBACK_FILE = "cat-default";
export const FALLBACK_FILE_ALT = "cat-other";

/** 解析顺序：categoryKey → iconKey → cat-default（组件加载失败再退 cat-other，
 *  最终退现有首字色块）。 */
export function resolveCatIconFile(opts: {
  categoryKey?: string | null;
  iconKey?: string | null;
}): string {
  const byKey = opts.categoryKey && CAT_KEY_TO_FILE[opts.categoryKey];
  if (byKey) return byKey;
  const byIcon = opts.iconKey && ICON_KEY_TO_FILE[opts.iconKey];
  if (byIcon) return byIcon;
  return FALLBACK_FILE;
}

/** 位图档位：生产落位 64/128/256 三档（源另有 512 档未落位——96px 以上
 *  暂无消费位，省去约 60% 体积；需要时从 design/icons 源补拷即可）。
 *  SVG 封装（164–202KB）仅作预览/存档，生产一律走 PNG 档。
 *  选中态：active=true 取 <file>-active-<tier>.png（两套均已补资产：
 *  aurora 主色加深一档白剪影反白；shanhaijing 金箔描边加亮+微暖底，
 *  与默认态同剪影同尺寸，切换不跳位）。 */
export function catIconUrl(
  set: CatIconSetId,
  file: string,
  px: number = 22,
  active: boolean = false,
): string {
  const tier = px <= 24 ? 64 : px <= 48 ? 128 : 256;
  const stem = active ? `${file}-active` : file;
  return `/cat-icons/${set}/${stem}-${tier}.png`;
}
