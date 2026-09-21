import type { MetadataRoute } from "next";
import { getSiteProfile } from "@/lib/site-profile";

// PWA manifest 改为运行时生成：让「安装到桌面」的应用名跟随站长自定义品牌（site_profile.brand）。
// force-dynamic：品牌运行时可改（后台改 site_name），不能被构建期预渲染写死。
export const dynamic = "force-dynamic";

export default async function manifest(): Promise<MetadataRoute.Manifest> {
  const profile = await getSiteProfile();
  const name = profile.brand || "FluxTorrent";
  return {
    name,
    short_name: name,
    description: profile.site_desc || "私有种子社区 —— 种下种子，一起成长",
    id: "/",
    start_url: "/",
    scope: "/",
    display: "standalone",
    orientation: "portrait-primary",
    background_color: "#F6FBFF",
    theme_color: "#2FA8FF",
    lang: "zh-CN",
    dir: "ltr",
    categories: ["productivity"],
    icons: [
      { src: "/icons/icon-192.png", sizes: "192x192", type: "image/png" },
      { src: "/icons/icon-512.png", sizes: "512x512", type: "image/png" },
      {
        src: "/icons/maskable-192.png",
        sizes: "192x192",
        type: "image/png",
        purpose: "maskable",
      },
      {
        src: "/icons/maskable-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
    shortcuts: [
      {
        name: "资源库",
        url: "/torrents",
        icons: [{ src: "/icons/icon-192.png", sizes: "192x192" }],
      },
      {
        name: "每日签到",
        url: "/my",
        icons: [{ src: "/icons/icon-192.png", sizes: "192x192" }],
      },
      {
        name: "发布资源",
        url: "/upload",
        icons: [{ src: "/icons/icon-192.png", sizes: "192x192" }],
      },
    ],
  };
}
