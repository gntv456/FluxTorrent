import { NextResponse, type NextRequest } from "next/server";

/**
 * 站点准入（PT 惯例）：未登录只放行登录页与静态资源，其余一律 302 → /login?next=…。
 * 会话判断用 flux.session 标记 cookie。P1 收敛后真凭证是 HttpOnly flux_token
 * （根路径 /，登录 Set-Cookie 下发），JS 不可读；middleware 只做页面准入。
 */

const PUBLIC_PATHS = [
  "/login",
  "/register",
  "/forgot",
  "/reset",
  "/offline",
  "/appeals",
  "/ban-log",
  "/resend",
  "/rules",
  "/faq",
  "/setup", // 安装向导（U3 §8.3）：冷启动期管理员未登录也要能进入
];

function isAnonymousOk(pathname: string): boolean {
  return (
    pathname.startsWith("/_next/") ||
    pathname.startsWith("/icons/") ||
    pathname.startsWith("/p/") ||
    [
      "/favicon.ico",
      "/manifest.webmanifest",
      "/sw.js",
      // 爬虫必读的机器文件必须匿名可达：否则准入闸门会把它们 307 到 /login，
      // 「允许收录」开关与 robots/sitemap 全成了摆设（0201 复验时抓到）
      "/robots.txt",
      "/sitemap.xml",
    ].includes(pathname)
  );
}

export function middleware(req: NextRequest) {
  const { pathname, search } = req.nextUrl;
  const hasSession = Boolean(req.cookies.get("flux.session")?.value);

  // API 经 rewrites 转发，不属页面路由 —— 鉴权由 API 侧 Bearer 校验，middleware 不拦
  if (pathname.startsWith("/api/")) {
    return NextResponse.next();
  }

  const publicPage =
    isAnonymousOk(pathname) || PUBLIC_PATHS.includes(pathname);
  if (hasSession || publicPage) {
    // 已登录访问登录页 → 回资源库，避免原地打转
    if (hasSession && pathname === "/login") {
      return NextResponse.redirect(new URL("/torrents", req.url));
    }
    return NextResponse.next();
  }

  const login = new URL("/login", req.url);
  const next = `${pathname}${search}`;
  if (next && next !== "/") login.searchParams.set("next", next);
  return NextResponse.redirect(login);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image).*)"],
};
