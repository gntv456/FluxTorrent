import { redirect } from "next/navigation";
import { LEGACY_TOOL } from "../_parts/admin-shared";

/**
 * 后台路径式深链兜底（P2，2026-09-26）。
 *
 * 后台工具走 `/admin?tool=<tab_key>` 客户端路由；但用户/外链常按直觉敲
 * `/admin/torrents`、`/admin/audit` 这类"路径式"地址——这些并非真实路由，
 * 此前直接落 Next 默认 404。本站真实子路由（settings / forums / users/[id]）
 * 由静态路由优先匹配，不会被本 catch-all 吞掉；其余未知 `/admin/*` 一律
 * 重定向到 `/admin?tool=<末段>`，并在末尾走 LEGACY_TOOL 映射（旧命名不失效）。
 * 未知 tab_key 由 page.tsx 的深链兜底落进概览，最终不留死链。
 */
export default async function AdminCatchAllPage({
  params,
}: {
  params: Promise<{ slug: string[] }>;
}) {
  const { slug } = await params;
  const last = (slug?.[slug.length - 1] ?? "").trim();
  const tool = LEGACY_TOOL[last] ?? last;
  redirect(tool ? `/admin?tool=${encodeURIComponent(tool)}` : "/admin");
}
