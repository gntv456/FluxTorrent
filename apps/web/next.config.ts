import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  // 方案 §8.1：前端仅公开配置
  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080",
  },
};

export default nextConfig;
