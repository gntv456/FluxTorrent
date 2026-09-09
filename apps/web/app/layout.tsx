import type { Metadata } from "next";
import "./globals.css";
import { getDict } from "@/i18n/server";
import { LocaleProvider } from "@/i18n/client";

export async function generateMetadata(): Promise<Metadata> {
  const { dict } = await getDict();
  return {
    title: `FluxTorrent · ${dict.meta.titleSuffix}`,
    description: dict.meta.description,
    manifest: "/manifest.webmanifest",
    icons: {
      icon: "/icons/icon-192.png",
      apple: "/icons/icon-192.png",
    },
    appleWebApp: {
      capable: true,
      statusBarStyle: "default",
      title: dict.common.brand,
    },
  };
}

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

export default async function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  const { dict, locale } = await getDict();
  return (
    <html lang={locale}>
      <body>
        <LocaleProvider dict={dict} locale={locale}>
          {children}
          <ServiceWorkerRegister />
        </LocaleProvider>
      </body>
    </html>
  );
}
