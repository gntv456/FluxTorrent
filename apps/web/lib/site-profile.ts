import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

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
  categories: { id: number; name: string }[];
  modules: Record<string, boolean>;
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
