/** 从 JWT 的 exp 反推「还剩多少秒」，供会话标记 cookie 用。
 *
 * 「记住登录」把后端 flux_token 的有效期拉到 30 天之后，前端那枚只给
 * middleware 判存在性的 flux.session 必须跟着一起长——否则 12 小时一到，
 * middleware 就把还持有有效票的人当游客踢回 /login，remember 形同虚设。
 *
 * 时长不在这里写死：读服务端刚发回来的那枚票的 exp，两边不可能漂。
 * 解不出（非三段式 / base64url 坏 / 无 exp / 已过期）返回 undefined，
 * 调用方回落到默认 12h——判不准时取窄的那头，不凭空放宽。
 *
 * 只做解码不做验签：验签是服务端的事，这里只要一个自报的到期时刻。 */
export function tokenSecondsLeft(jwt?: string): number | undefined {
  if (!jwt) return undefined;
  const parts = jwt.split(".");
  if (parts.length < 2) return undefined;
  try {
    const b64 = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const claims = JSON.parse(atob(b64)) as { exp?: unknown };
    if (typeof claims.exp !== "number") return undefined;
    const left = Math.floor(claims.exp - Date.now() / 1000);
    return left > 0 ? left : undefined;
  } catch {
    return undefined;
  }
}
