import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  // 方案 §8.1：前端仅公开配置
  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL ?? "",
  },
  // 浏览器请求走同源 /api → 服务端转发（免 CORS、免暴露内部端口）
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
      // HSTS 经 TLS 终结的反代透传后生效；完整 CSP 需 nonce 基建（Next 内联脚本）暂缓
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
        ],
      },
    ];
  },
};

export default nextConfig;
