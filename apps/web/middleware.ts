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
  // 邮箱激活页必须匿名可达（P-108）：未激活的新用户本来就登不进（登录被
  // guard_login 拦），若准入闸门再把他弹去 /login，邮件里的链接就永远走不到
  // —— `/reset` 在表里而 `/email-verify` 漏了，等于激活链最后一步是死的。
  "/email-verify",
  // 申请制入站（0227）：本页的定位就是「匿名提交举证」，此前漏在表外，
  // 未登录访客点「申请邀请码」会被弹回登录页——申请通道等于死链。
  "/apply",
  "/offline",
  "/appeals",
  "/ban-log",
  "/rules",
  "/faq",
  "/setup", // 安装向导（U3 §8.3）：冷启动期管理员未登录也要能进入
  // 维护页（0462）：maintenance_mode=yes 时 (main) 布局把非 staff 302 到
  // 这里，它自己必须匿名可达，否则死循环重定向
  "/maintenance",
];

function isAnonymousOk(pathname: string): boolean {
  return (
    pathname.startsWith("/_next/") ||
    pathname.startsWith("/icons/") ||
    // 自托管字体（public/fonts/*.woff2）必须匿名可达：被 307 成登录页 HTML 时
    // 浏览器只报「字体解码失败」，字库静默回退成系统字体，界面看不出破口
    pathname.startsWith("/fonts/") ||
    // 内置品牌资产（站标/字标/OG 卡）：登录页品牌区与爬虫 OG 抓取都要匿名可达，
    // 否则站标图被 307 成登录页 HTML、分享卡抓取落空
    pathname.startsWith("/brand/") ||
    // 内置背景图（0445/0447 的三张默认样图）：站长自托管在站内时，看的人正是
    // **未登录**访客（登录页背景），被 307 成登录页 HTML 后浏览器只会静默丢图
    pathname.startsWith("/backgrounds/") ||
    // 勋章图（public/medals/*）：勋章墙/勋章殿堂在未登录页也会渲染，
    // 被 307 成登录页 HTML 时浏览器只报「图片加载失败」，勋章位整片空白
    pathname.startsWith("/medals/") ||
    pathname.startsWith("/p/") ||
    [
      "/favicon.ico",
      "/manifest.webmanifest",
      "/sw.js",
      // 爬虫必读的机器文件必须匿名可达：否则准入闸门会把它们 307 到 /login，
      // 「允许收录」开关与 robots/sitemap 全成了摆设（0201 复验时抓到）
      "/robots.txt",
      "/sitemap.xml",
      // 健康检查（0224 G30）：LB/compose 探针无会话，被 307 到 /login 等于
      // 判活永远拿不到 200
      "/health",
    ].includes(pathname)
  );
}

/**
 * 维护开关（0462 补漏）：middleware 跑在页面准入最前面，未登录访客
 * 此前一律 307 /login，而 /login 用纯净布局、不挂 (main) 的维护门，
 * 于是「开了维护模式仍看到登录页」。开关从公开端点 site-profile 读
 * （该端点在 API 维护闸白名单里），30s TTL + fail-open：读挂按未维护
 * 放行，避免 API 抖动把访客误关进维护页。
 */
let maintCached: { on: boolean; until: number } | null = null;

async function siteMaintaining(req: NextRequest): Promise<boolean> {
  const now = Date.now();
  if (maintCached && now < maintCached.until) return maintCached.on;
  let on = false;
  try {
    // 优先直连容器网（compose 给 web 注入 API_SERVER_URL）；缺失时
    // 回落同源 /api（经 rewrites 转发），本地开发同样可用。
    const base = process.env.API_SERVER_URL?.replace(/\/$/, "");
    const url = base
      ? `${base}/api/v1/site-profile`
      : new URL("/api/v1/site-profile", req.url).toString();
    const res = await fetch(url, {
      signal: AbortSignal.timeout(1500),
      cache: "no-store",
    });
    const body = (await res.json()) as {
      data?: { maintenance?: { on?: boolean } };
    };
    on = Boolean(body?.data?.maintenance?.on);
  } catch {
    on = false;
  }
  maintCached = { on, until: now + 30_000 };
  return on;
}

export async function middleware(req: NextRequest) {
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

  // 维护中：匿名访客落维护页，而不是登录页。/login 仍在 PUBLIC_PATHS
  // 里，staff 从维护页的「登录」入口进去照常登录（文档口径）。
  if (await siteMaintaining(req)) {
    return NextResponse.redirect(new URL("/maintenance", req.url));
  }

  const login = new URL("/login", req.url);
  const next = `${pathname}${search}`;
  if (next && next !== "/") login.searchParams.set("next", next);
  return NextResponse.redirect(login);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image).*)"],
};
