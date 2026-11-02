/**
 * 站点/登录页背景图（0445）：把后台配置的 URL 变成安全的 CSS 值。
 *
 * 设置项走 `settings_meta` 的 rule=url_or_path（0456）：收 http(s) 绝对地址
 * 或**站内相对路径**（形如 /backgrounds/x.jpg）。相对路径是默认推荐写法——
 * 背景随访问入口同源加载，不必把域名焊进配置（实测：写死公网域名后，
 * 从本机访问要绕公网一圈，大图 36s 都拉不完，界面等于没有背景）。
 * 这里再守一道，拒绝引号/反斜杠/空白/超长/协议相对（//host）——值会拼进
 * `url("…")`，不能让管理员（或被篡改的配置）注入额外 CSS。
 * host:port 形态（无协议）无法直接给浏览器用，按不启用处理。
 */

const DISALLOWED = /["'\\\s]/;

/** URL → `url("…")`；非法/空 → null（不渲染背景层） */
export function cssBackgroundImage(
  url: string | null | undefined,
): string | null {
  const u = (url ?? "").trim();
  // 站内相对路径：单斜杠开头；//host 是协议相对写法，按外部地址拒绝
  const relative = /^\/(?![/\\])/.test(u);
  if (!relative && !/^https?:\/\//i.test(u)) return null;
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
