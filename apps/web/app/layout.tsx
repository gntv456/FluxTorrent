import type { Metadata } from "next";
import "./globals.css";
import { Header, MobileTabBar } from "@/components/layout";

export const metadata: Metadata = {
  title: "FluxTorrent · 好学",
  description: "教育资源私有种子社区 —— 种下种子，一起成长",
  manifest: "/manifest.webmanifest",
  icons: {
    icon: "/icons/icon-192.png",
    apple: "/icons/icon-192.png",
  },
  appleWebApp: {
    capable: true,
    statusBarStyle: "default",
    title: "好学",
  },
};

/** M26 PWA：注册 Service Worker（仅生产，避免开发态热更被缓存干扰） */
function ServiceWorkerRegister() {
  if (process.env.NODE_ENV !== "production") return null;
  return (
    <script
      dangerouslySetInnerHTML={{
        __html: `if('serviceWorker' in navigator){window.addEventListener('load',function(){navigator.serviceWorker.register('/sw.js').catch(function(){/* SW 失败不阻塞站点 */})})}`,
      }}
    />
  );
}

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="zh-CN">
      <body>
        {/* 桌面顶栏 64px 吸顶（设计稿 §4.1） */}
        <Header />
        <main className="mx-auto w-full max-w-[1280px] px-4 pb-24 pt-6 md:px-6">
          {children}
        </main>
        {/* 移动底部 5 Tab（设计稿：首页/搜索/发布/消息/个人中心） */}
        <MobileTabBar />
        <ServiceWorkerRegister />
      </body>
    </html>
  );
}
