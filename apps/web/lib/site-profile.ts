import { cache } from "react";
import { api } from "@/lib/api-client";

export const dynamic = "force-dynamic";

export interface SiteProfile {
  site_type: string;
  pack_name: string | null;
  brand: string;
  /** 登录页品牌区标语（0143）：站型包默认 ← site_settings.site_tagline 覆盖；空回落 i18n 字典 */
  tagline?: string;
  /** 站点 Logo URL（site_settings.site_logo；空 = 前端回落占位图形） */
  site_logo?: string | null;
  /** 站点货币名（0082）：默认「魔力」，站长后台 site_settings.currency_name 可改 */
  currency_name?: string;
  /** 建站日期（site_settings.datefounded，页脚版权条用） */
  founded?: string | null;
  /** 启用的元数据源（0087，site_settings.metadata_sources；控制条目输入/PT-Gen 显隐） */
  metadata_sources?: string[];
  /** 站点简介（0088，site_settings.site_desc；页脚「站点信息」，空回落字典默认） */
  site_desc?: string | null;
  /** 字幕区口径（0146）：lyric = 歌词站（lrc 白名单/隐藏 FPS/歌词文案） */
  subtitle_kind?: string;
  /** 字幕区显示名（0146；默认「字幕」，音乐站「歌词」） */
  subtitle_label?: string;
  /** 语言切换器显隐（0209：locale_switcher_enabled=no 隐藏，单语站） */
  locale_switcher_enabled?: string;
  categories: {
    id: number;
    name: string;
    icon_key?: string;
    /** 分类色（0183）：#rrggbb，由 categories 表下发 */
    bg_color?: string | null;
  }[];
  /** 旧三列（torrents.medium_id / grade_id / edition_id）的词表，id 以实体表为准。
   *  与新模型的 section_dict.id 不是同一套编号，不可互换。 */
  torrent_dicts?: {
    grades?: DictEntry[];
    media?: DictEntry[];
    editions?: DictEntry[];
  };
  modules: Record<string, boolean>;
  /** 主题令牌（0189）：有值的 theme_token_* 键 → #rrggbb；layout 注入 :root */
  theme_tokens?: Record<string, string>;
  /** SEO（0201）：META 描述/关键词与「是否允许收录」——前台与 RSS 共用同一份描述 */
  seo?: {
    description?: string;
    keywords?: string;
    indexable?: boolean;
  };
  /** 术语规则（0205 四审 L7）：字典出口按此改写固有词；缺省/空数组 = 不改写 */
  terms?: { canonical: string; replacement: string }[];
}

/** 站型字典条目（id 由后端词表表给出，前端不做任何下标补偿） */
export interface DictEntry {
  id: number;
  name: string;
}

/** 公开：站点档案（RSC 服务端获取，layout 与上传表单复用；失败回落 general 默认） */
export async function getSiteProfile(): Promise<SiteProfile> {
  try {
    return await api.get<SiteProfile>("/api/v1/site-profile");
  } catch {
    return {
      site_type: "general",
      pack_name: null,
      brand: "",
      tagline: "",
      site_logo: null,
      currency_name: "魔力",
      founded: null,
      metadata_sources: ["imdb", "douban", "bangumi", "indienova"],
      site_desc: null,
      subtitle_kind: "subtitle",
      subtitle_label: "字幕",
      categories: [],
      modules: {},
    };
  }
}

/** 站型字典（分类 + 旧三列）的唯一真值源：一律来自后端，前端不持有词表。
 *  取不到就是空表——显示侧回落 `#id`，筛选侧只剩「全部」，
 *  绝不拿另一套硬编码词表顶替（那是分类显示 bug 的源头）。
 *  cache()：表行/卡片每行都要名字，同一请求内只取一次档案。 */
export const getTorrentDicts = cache(
  async (): Promise<{
    categories: DictEntry[];
    grades: DictEntry[];
    media: DictEntry[];
    editions: DictEntry[];
    /** 分类色（0183）：id → #rrggbb，来自 categories.bg_color */
    colors: Record<number, string>;
  }> => {
    const p = await getSiteProfile();
    const cats = p.categories ?? [];
    return {
      categories: cats.map((c) => ({ id: c.id, name: c.name })),
      grades: p.torrent_dicts?.grades ?? [],
      media: p.torrent_dicts?.media ?? [],
      editions: p.torrent_dicts?.editions ?? [],
      colors: colorMap(cats),
    };
  },
);

/** 分类色映射：categories.bg_color → {id: #rrggbb} */
export function colorMap(
  cats: { id: number; bg_color?: string | null }[],
): Record<number, string> {
  return Object.fromEntries(
    cats.filter((c) => c.bg_color).map((c) => [c.id, c.bg_color as string]),
  );
}

/** 档案取不到颜色时的中性兜底（不再是「按 id 硬编码一张色表」） */
export const CAT_FALLBACK_COLOR = "#93a1bc";

export function catColor(
  colors: Record<number, string>,
  id: number | null | undefined,
): string {
  if (id === null || id === undefined) return CAT_FALLBACK_COLOR;
  return colors[id] ?? CAT_FALLBACK_COLOR;
}

/** 字典列表 → id:名称 映射 */
export function byId(list: DictEntry[]): Record<number, string> {
  return Object.fromEntries(list.map((d) => [d.id, d.name]));
}

/** 按 id 取显示名；缺词条宁可显示 `#id`，也不用错词表糊上去 */
export function dictName(
  map: Record<number, string>,
  id: number | null | undefined,
): string {
  if (id === null || id === undefined) return "";
  return map[id] ?? `#${id}`;
}

/** 旧三列（媒介/学段/版本）维度的显示名。
 *  词表为空 = 该维度在本站型已退役（0180 按站型收敛，非教育站不再下发
 *  grades/editions），此时整项不显示——把 `#11` 这种内部 id 抖给用户是泄漏；
 *  词表非空但缺这个 id 才是真数据异常，保留 `#id` 让人看得见。 */
export function legacyDimName(
  list: DictEntry[],
  id: number | null | undefined,
): string {
  if (id === null || id === undefined || list.length === 0) return "";
  return dictName(byId(list), id);
}
