import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

export interface SiteProfile {
  site_type: string;
  pack_name: string | null;
  brand: string;
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
      categories: [],
      modules: {},
    };
  }
}
