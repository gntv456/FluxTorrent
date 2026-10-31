/**
 * 站点/登录页背景图（0445）：把后台配置的 URL 变成安全的 CSS 值。
 *
 * 设置项走 `settings_meta` 的 rule=url（后端只收 http(s) 或 host:port），
 * 这里再守一道：只认 http(s) 绝对地址，拒绝引号/反斜杠/空白/超长——
 * 值会拼进 `url("…")`，不能让管理员（或被篡改的配置）注入额外 CSS。
 * host:port 形态（无协议）无法直接给浏览器用，按不启用处理。
 */

const DISALLOWED = /["'\\\s]/;

/** URL → `url("…")`；非法/空 → null（不渲染背景层） */
export function cssBackgroundImage(
  url: string | null | undefined,
): string | null {
  const u = (url ?? "").trim();
  if (!/^https?:\/\//i.test(u)) return null;
  if (u.length > 2048 || DISALLOWED.test(u)) return null;
  return `url("${u}")`;
}

/** 0~100 不透明度：非数字回落，越界夹紧 */
export function backgroundOpacity(
  n: number | null | undefined,
  fallback: number,
): number {
  const v = Number.isFinite(n) ? Number(n) : fallback;
  return Math.min(100, Math.max(0, v));
}
