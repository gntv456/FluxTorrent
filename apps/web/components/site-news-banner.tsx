import { api } from "@/lib/api-client";

/** 服务端取数壳：数据取好交给 client 横幅（关闭态需 localStorage）。
 *  取数失败静默隐藏，不影响整站可用。 */
export async function SiteNewsBanner() {
  let top: { id: number; title: string } | null = null;
  try {
    const r = await api.get<{ news?: { id: number; title: string }[] }>(
      "/api/v1/home",
      300,
    );
    top = r?.news?.[0] ?? null;
  } catch {
    top = null;
  }
  if (!top) return null;
  const mod = await import("./site-news-banner-client");
  return <mod.SiteNewsBanner top={top} />;
}
