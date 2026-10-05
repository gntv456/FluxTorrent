import { redirect } from "next/navigation";

export const dynamic = "force-dynamic";

/**
 * /users/{id}/torrentlist 深链转发（0283 P2）：API 路径
 * /api/v1/users/{id}/torrentlist 一直存在，但页面路由缺失——排行榜等
 * 处直接拼此路径会 404。统一转发到用户主页的种子 tab（?tab=torrents）。
 */
export default async function UserTorrentlistRedirect({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  redirect(`/users/${encodeURIComponent(id)}?tab=seeding`);
}
