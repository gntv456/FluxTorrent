import { redirect } from "next/navigation";
import { api } from "@/lib/api-client";

export const dynamic = "force-dynamic";

/**
 * `/me` 是父路径（只有 `/me/achievements`、`/me/exams` 两个子页），
 * 直接访问会 404 —— 但用户直觉会敲 `/me`，这里补一层重定向到自己的公开档案页。
 *
 * ⚠️ `redirect()` 靠抛异常实现跳转，**必须放在 try/catch 之外**，
 * 被捕获会静默失效（catch 里只能处理取数失败）。
 */
export default async function MeIndexPage() {
  let uid: number | null = null;
  try {
    const me = await api.get<{ id: number }>("/api/v1/me");
    uid = typeof me.id === "number" ? me.id : null;
  } catch {
    uid = null;
  }
  if (uid === null) redirect("/login?next=/me");
  redirect(`/users/${uid}`);
}
