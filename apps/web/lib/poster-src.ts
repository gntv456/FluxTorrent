/**
 * 海报外链代理改写（2026-11-02）：豆瓣图床收紧防盗链——无 Referer 直接
 * 418（旧版按 Referer 拦、no-referrer 可绕；新版反过来必须带 douban 域
 * Referer）。浏览器端给不了这个 Referer（referrerPolicy 只能选择带/不带，
 * 带的也是本站域），海报一律走 api 的 `/works/poster-proxy` 出站。
 *
 * 只改写图床白名单域（与 api 侧 works_poster_proxy.rs 的白名单同源）；
 * 其他外链（站内相对路径、未来新增图床）原样返回，避免把代理变成
 * 万能跳板。
 */

const PROXY_HOST_SUFFIXES = [
  ".doubanio.com",
  "image.tmdb.org",
] as const;

/** API 基址（与 api-client 同源口径：构建期烧入的 NEXT_PUBLIC_API_URL） */
function apiBase(): string {
  // 站内部署时 API 与 web 同源（rewrites 反代 /api/v1），空前缀即同源
  return process.env.NEXT_PUBLIC_API_URL ?? "";
}

/** 海报 URL → 代理 URL；非白名单域原样返回 */
export function posterSrc(url: string | null | undefined): string | null {
  if (!url) return null;
  // 相对路径 / data: / blob: 不走代理
  if (!/^https?:\/\//i.test(url)) return url;
  let host: string;
  try {
    host = new URL(url).host;
  } catch {
    return url;
  }
  const whitelisted = PROXY_HOST_SUFFIXES.some(
    (s) => host === s || host.endsWith(s),
  );
  if (!whitelisted) return url;
  const q = encodeURIComponent(url);
  return `${apiBase()}/api/v1/works/poster-proxy?url=${q}`;
}
