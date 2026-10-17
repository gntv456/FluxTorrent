import { hasSessionCookie } from "@/lib/api-client";

/** RSC 侧登录态判定（论坛首页两翼用它分「游客 / 取档失败 / 有数据」）。
 *
 *  为什么不和 `hasSessionCookie()` 同住在 lib/api-client.ts：那个文件
 *  压在 line_limit 基线（301/300）上、只许瘦不许胖，再加一个函数就把
 *  门禁顶红。口径与 api-client 的 request() 一致：token 是 HttpOnly 的
 *  flux_token，浏览器 JS 读不到，只有 next/headers 能取。 */
export async function hasSessionToken(): Promise<boolean> {
  if (typeof window !== "undefined") return hasSessionCookie();
  const { cookies } = await import("next/headers");
  return Boolean((await cookies()).get("flux_token")?.value);
}
