/** 站点对外基地址（与 api 侧 rss_http/donate 的 PUBLIC_SITE_URL 口径一致）。
 *  未配置时返回 null：调用方要么省掉绝对地址，要么明确降级，不编一个假域名。 */
export function siteBase(): URL | null {
  const raw = process.env.PUBLIC_SITE_URL?.trim();
  if (!raw) return null;
  try {
    const u = new URL(raw);
    return u.protocol === "http:" || u.protocol === "https:" ? u : null;
  } catch {
    return null;
  }
}
