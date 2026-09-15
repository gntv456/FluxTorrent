import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

export interface SiteProfile {
  site_type: string;
  pack_name: string | null;
  brand: string;
  /** 站点货币名（0082）：默认「魔力」，站长后台 site_settings.currency_name 可改 */
  currency_name?: string;
  /** 建站日期（site_settings.datefounded，页脚版权条用） */
  founded?: string | null;
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
      currency_name: "魔力",
      founded: null,
      categories: [],
      modules: {},
    };
  }
}
