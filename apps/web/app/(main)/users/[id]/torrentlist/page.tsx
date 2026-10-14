import { redirect } from "next/navigation";

/**
 * /users/{id}/torrentlist 深链转发（0283 P2）：API 路径
 * /api/v1/users/{id}/torrentlist 一直存在，但页面路由缺失——排行榜等
 * 处直接拼此路径会 404。统一转发到用户主页的种子 tab（?tab=torrents）。
 *
 * 纯转发路由（同 /users、/farm 口径）：redirect 只读 params 即可静态
 * 判定，无需 force-dynamic，也不属数据路由——loading 骨架无意义。
 */
export default async function UserTorrentlistRedirect({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  redirect(`/users/${encodeURIComponent(id)}?tab=seeding`);
}
