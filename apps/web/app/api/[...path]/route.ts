import type { NextRequest } from "next/server";

/**
 * /api 运行期反向代理（0225 G30-B4）。
 *
 * 现状缺口（附录 D 档三）：next.config.ts 的 rewrites 在 `next build` 时把
 * API_SERVER_URL 固化进 routes-manifest——想换 api 上游只能重打镜像（或
 * --add-host 劫持解析），滚动发布/多上游切换做不了。
 *
 * 改造：catch-all route 在**运行期**读 API_SERVER_URL 转发，环境变量一改
 * （容器重启即可）上游就换。next.config.ts 的 rewrites 保留为兜底——route
 * 优先级高于 rewrites，二者不冲突；rewrites 仅在本 route 被移除时生效。
 *
 * 语义对齐 rewrites：路径原样透传（/api/:path* → {upstream}/api/:path*），
 * method/headers/body 全透传，响应原样返回（含 Set-Cookie/流式附件下载）。
 */

const UPSTREAM = () =>
  process.env.API_SERVER_URL ?? "http://localhost:8080";

async function proxy(req: NextRequest) {
  const { pathname, search } = req.nextUrl;
  const target = `${UPSTREAM()}${pathname}${search}`;
  // hop-by-hop 头不透传（host/connection 等）
  const headers = new Headers();
  req.headers.forEach((v, k) => {
    if (
      !["host", "connection", "keep-alive", "transfer-encoding"].includes(k)
    ) {
      headers.set(k, v);
    }
  });
  headers.set("x-forwarded-host", req.headers.get("host") ?? "");
  // XFF 三轮审计（2026-10-07）：不再从请求头「猜」clientIp 放首位重组——
  // 旧版取 x-real-ip ?? XFF 首值，直连 web 形态下这两者都是攻击者可设的
  // 任意值，重组后 api 按 XFF 右值取到假 IP（绕 ip_bans/限流）。
  // 改为原样透传 XFF：api 的 TRUST_PROXY 语义（右值=直连我的那台代理追加
  // 的值）在宝塔形态（nginx→web→api）下取到 nginx 追加的真实客户端；
  // 直连 web 形态下 XFF 是攻击者自带的假值——但那种形态 TRUST_PROXY 本就
  // 不该开（api 用 socket 对端），不构成绕过。x-real-ip 原样透传（若外层
  // 可信 nginx 设置了它，api 侧未来可用）。
  const xff = req.headers.get("x-forwarded-for");
  if (xff) headers.set("x-forwarded-for", xff);

  const init: RequestInit = {
    method: req.method,
    headers,
    redirect: "manual",
  };
  if (!["GET", "HEAD"].includes(req.method)) {
    init.body = await req.arrayBuffer();
  }
  try {
    const upstream = await fetch(target, init);
    // Response 构造：status/headers 透传，body 流式（附件下载不被整个缓冲）
    const respHeaders = new Headers();
    upstream.headers.forEach((v, k) => {
      if (k.toLowerCase() !== "transfer-encoding") {
        respHeaders.set(k, v);
      }
    });
    return new Response(upstream.body, {
      status: upstream.status,
      statusText: upstream.statusText,
      headers: respHeaders,
    });
  } catch (e) {
    return new Response(
      JSON.stringify({
        code: 1003,
        message: "网关无法连接 API 上游（检查 API_SERVER_URL）",
        data: null,
        request_id: "",
      }),
      {
        status: 502,
        headers: { "content-type": "application/json" },
      },
    );
  }
}

export {
  proxy as GET,
  proxy as POST,
  proxy as PUT,
  proxy as DELETE,
  proxy as PATCH,
  proxy as HEAD,
  proxy as OPTIONS,
};
