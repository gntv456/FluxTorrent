import { redirect } from "next/navigation";

/**
 * `/users` 是父路径（只有 `/users/[id]`），直接访问会 404。
 * 重定向到排行榜 —— 那是全站唯一的「用户列表」语义页。
 */
export default function UsersIndexPage() {
  redirect("/top");
}
