import type { NextRequest } from "next/server";
import { NextResponse } from "next/server";

/**
 * B 站分享短链解析（b23.tv → 完整视频页 URL）。
 *
 * 只允许 b23.tv 进、只回 bilibili.com/video/... 出：重定向链逐跳手判，
 * 不跟随任意 Location（避免把同源服务端当 SSRF 跳板）。解析失败一律回
 * `{ url: null }`，前端把原短链当普通链接插入，不阻塞发帖。
 */

const SHORT_HOST = "b23.tv";
const CACHE_MS = 60 * 60 * 1000;
const RATE_WINDOW_MS = 60 * 1000;
const RATE_MAX = 60;

const cache = new Map<string, { url: string | null; at: number }>();
const hits = new Map<string, { n: number; reset: number }>();

function reply(url: string | null, status = 200) {
  return NextResponse.json(
    { url },
    {
      status,
      headers: {
        "cache-control": url ? "public, max-age=3600" : "no-store",
      },
    },
  );
}

/** 短链入口校验：只认 b23.tv 的字母数字路径。 */
function shortUrl(raw: string): URL | null {
  try {
    const u = new URL(raw);
    if (u.protocol !== "https:") return null;
    if (u.hostname.toLowerCase() !== SHORT_HOST) return null;
    if (!/^\/[A-Za-z0-9_-]+$/.test(u.pathname)) return null;
    return u;
  } catch {
    return null;
  }
}

/** 只接受完整视频页 URL，输出站内统一 canonical 形态。 */
function canonicalBilibili(u: URL): string | null {
  if (u.protocol !== "https:") return null;
  const host = u.hostname.toLowerCase();
  if (!["bilibili.com", "www.bilibili.com", "m.bilibili.com"].includes(host))
    return null;
  const m = u.pathname.match(
    /^\/video\/((?:BV[0-9A-Za-z]+)|(?:av[0-9]+))\/?$/,
  );
  return m ? `https://www.bilibili.com/video/${m[1]}` : null;
}

async function resolve(key: string): Promise<string | null> {
  let cur = new URL(key);
  for (let hop = 0; hop < 4; hop++) {
    const r = await fetch(cur, {
      method: "HEAD",
      redirect: "manual",
      cache: "no-store",
      signal: AbortSignal.timeout(5000),
      headers: { "user-agent": "Mozilla/5.0 (compatible; FluxTorrent)" },
    });
    if (r.status < 300 || r.status >= 400) return null;
    const loc = r.headers.get("location");
    if (!loc) return null;
    const next = new URL(loc, cur);
    const canonical = canonicalBilibili(next);
    if (canonical) return canonical;
    if (
      next.protocol !== "https:" ||
      next.hostname.toLowerCase() !== SHORT_HOST
    )
      return null;
    cur = next;
  }
  return null;
}

function rateLimited(ip: string): boolean {
  const now = Date.now();
  const rec = hits.get(ip);
  if (!rec || now >= rec.reset) {
    hits.set(ip, { n: 1, reset: now + RATE_WINDOW_MS });
    return false;
  }
  rec.n++;
  return rec.n > RATE_MAX;
}

export async function GET(req: NextRequest) {
  const raw = req.nextUrl.searchParams.get("url") ?? "";
  const u = shortUrl(raw);
  if (!u) return reply(null);

  // 分享参数不进缓存键/请求：同一短链只解一次。
  const key = `https://${SHORT_HOST}${u.pathname}`;
  const now = Date.now();
  const hit = cache.get(key);
  if (hit && now - hit.at < CACHE_MS) return reply(hit.url);

  // 限流只防批量刷出口；XFF 在直连形态可伪造，但这里不承担鉴权。
  const ip =
    req.headers.get("x-forwarded-for")?.split(",")[0]?.trim() ||
    req.headers.get("x-real-ip") ||
    "local";
  if (rateLimited(ip)) return reply(null, 429);

  let url: string | null = null;
  try {
    url = await resolve(key);
  } catch {
    url = null;
  }
  if (cache.size > 500) {
    for (const [k, v] of cache) {
      if (now - v.at >= CACHE_MS) cache.delete(k);
    }
  }
  if (hits.size > 1000) {
    for (const [k, v] of hits) {
      if (now >= v.reset) hits.delete(k);
    }
  }
  // 失败不入缓存：瞬时网络故障不该让同一短链一小时都用不了。
  if (url) cache.set(key, { url, at: now });
  return reply(url);
}
