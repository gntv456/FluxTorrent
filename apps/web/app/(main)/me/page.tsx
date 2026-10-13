import { redirect } from "next/navigation";
import { api } from "@/lib/api-client";

export const dynamic = "force-dynamic";

/**
 * `/me` 是父路径（只有 `/me/achievements`、`/me/exams` 两个子页），
 * 直接访问会 404 —— 但用户直觉会敲 `/me`，这里补一层重定向到自己的公开档案页。
 *
 * 四态核销（DS-PLAN-01 W-26 / A3）：本页是**纯转发路由**——无数据 UI、
 * 无写操作，四态中只有「错误」有意义且已按最严格形态处理：取 uid 失败
 * （含未登录 401）不渲染错误面板，直接 redirect 登录页带回跳（next=/me），
 * 登录后自动回本页再跳档案。加载态由路由组 `(main)` 的框架壳承担，
 * 空态/成功态对无内容页不适用。真正的用户中心四态在 `/my` 与
 * `/users/[id]`（后者已有 loading.tsx + 404/5xx 分流）。
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
