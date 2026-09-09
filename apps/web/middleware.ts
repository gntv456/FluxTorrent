import { NextResponse, type NextRequest } from "next/server";

/**
 * 站点准入（PT 惯例）：未登录只放行登录页与静态资源，其余一律 302 → /login?next=…。
 * 会话判断用 flux.session cookie（登录时与 localStorage token 同步写入；
 * HttpOnly 仅作存在性标记，真正鉴权仍是请求头 Bearer token）。
 */

const PUBLIC_PATHS = ["/login", "/register", "/forgot", "/offline"];

function isAsset(pathname: string): boolean {
  return (
    pathname.startsWith("/_next/") ||
    pathname.startsWith("/icons/") ||
    ["/favicon.ico", "/manifest.webmanifest", "/sw.js"].includes(pathname)
  );
}

export function middleware(req: NextRequest) {
  const { pathname, search } = req.nextUrl;
  const hasSession = Boolean(req.cookies.get("flux.session")?.value);

  if (hasSession || isAsset(pathname) || PUBLIC_PATHS.includes(pathname)) {
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
