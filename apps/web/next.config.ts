import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  // 方案 §8.1：前端仅公开配置
  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL ?? "",
  },
  // 浏览器请求走同源 /api → 服务端转发（免 CORS、免暴露内部端口）。
  // 0225 G30-B4：app/api/[...path]/route.ts 的运行期代理优先级更高（本
  // rewrites 是它被移除时的兜底；且 rewrites 在构建期固化 API_SERVER_URL，
  // 运行期换上游须靠那个 route）。
  async rewrites() {
    const api = process.env.API_SERVER_URL ?? "http://localhost:8080";
    return [{ source: "/api/:path*", destination: `${api}/api/:path*` }];
  },
  // 认证/交互关键页 HTML 不缓存：防旧 SW/代理留下过期 shell（点击无反应类问题）
  async headers() {
    return [
      {
        source: "/:path(login|register|forgot|reset|offline)",
        headers: [{ key: "Cache-Control", value: "no-store, must-revalidate" }],
      },
      // 安全响应头基线（§5.7）：全站生效。
      // HSTS 经 TLS 终结的反代透传后生效；CSP（E1）：Next 内联引导脚本
      // （主题 no-flash / SW 注册 / 主题令牌注入）走 'unsafe-inline' 基线——
      // 这些脚本全在本仓 layout.tsx 内、无任何用户输入插值；自定义页面
      // body 为站长后台富文本，信任级别等同模板，故 style 也放 inline。
      // 升级 nonce 基建前，本基线至少斩断外域脚本/对象/框架注入面。
      {
        source: "/:path*",
        headers: [
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "X-Frame-Options", value: "DENY" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          {
            key: "Permissions-Policy",
            value: "camera=(), microphone=(), geolocation=()",
          },
          {
            key: "Strict-Transport-Security",
            value: "max-age=31536000; includeSubDomains",
          },
          {
            key: "Content-Security-Policy",
            value: [
              "default-src 'self'",
              // Next 运行时/水合脚本与引导内联脚本（见上注）
              "script-src 'self' 'unsafe-inline'",
              // 开发态连 HMR；生产不受影响（VerbatimModuleSyntax 无 eval）
              ...(process.env.NODE_ENV === "development"
                ? ["script-src-elem 'self' 'unsafe-inline' 'unsafe-eval' ws:"]
                : []),
              // Tailwind 产出与主题令牌 <style> 注入
              "style-src 'self' 'unsafe-inline'",
              // 附件/封面/头像可能来自站长配置的绝对 URL 或外域图床
              "img-src 'self' data: blob: http: https:",
              "font-src 'self' data:",
              // API 走同源 /api 代理（rewrites/运行期 route），无需 connect-src 外域
              "connect-src 'self'",
              // 站内上传与字幕/论坛媒体播放（Range 206 那套）
              "media-src 'self' blob:",
              // 论坛/公告视频内嵌白名单（embed_rules）按 frame-src 'self' 收口，
              // 外域 embed 站点如需生效，站长须在反向代理层为 /embed 路径放宽
              "frame-src 'self'",
              "object-src 'none'",
              "base-uri 'self'",
              "form-action 'self'",
              "frame-ancestors 'none'",
            ].join("; "),
          },
        ],
      },
    ];
  },
};

export default nextConfig;
