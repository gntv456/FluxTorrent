/** 论坛视频插入/解析的共享纯函数（组件侧只留 UI）。 */

/** 把输入框里的多枚链接按空白/逗号切开并去重（粘贴多行也能一次插入）。 */
export function splitVideoUrls(raw: string): string[] {
  const out: string[] = [];
  for (const part of raw.split(/[\s,，]+/)) {
    if (part && !out.includes(part)) out.push(part);
  }
  return out;
}

/** 写入正文：与上文之间留空行（解析器按行切块，空行只为源文可读）。 */
export function appendVideoBlock(body: string, text: string): string {
  const base = body.trimEnd();
  return `${base}${base ? "\n\n" : ""}${text}\n`;
}

const B23_RE = /^https?:\/\/b23\.tv\/[A-Za-z0-9]+/i;

/** B 站分享面板给的短链（b23.tv）——白名单规则只认完整视频页 URL。 */
export function isBilibiliShortUrl(url: string): boolean {
  return B23_RE.test(url.trim());
}

/** 同一 URL 只解析一次（含失败；失败原样返回，不阻塞发帖）。 */
const resolving = new Map<string, Promise<string>>();

/** b23.tv 短链 → 同源 /api/embed-resolve → canonical 视频页 URL。 */
export function resolveVideoUrl(url: string): Promise<string> {
  const u = url.trim();
  if (!isBilibiliShortUrl(u)) return Promise.resolve(u);
  const cached = resolving.get(u);
  if (cached) return cached;
  const p = (async () => {
    try {
      const r = await fetch(
        `/api/embed-resolve?url=${encodeURIComponent(u)}`,
        { cache: "no-store" },
      );
      const j = (await r.json()) as { url?: string | null };
      return typeof j.url === "string" && j.url ? j.url : u;
    } catch {
      return u;
    }
  })();
  resolving.set(u, p);
  return p;
}
