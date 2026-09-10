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

/** M26 PWA：注册 Service Worker（仅生产，避免开发态热更被缓存干扰）。
 *  发现新 SW 安装完成即令其 skipWaiting + 本页刷新一次，保证发版后
 *  用户最多手动刷新一次就能拿到新 bundle（避免旧 shell 引用已 404 的 chunk
 *  导致"页面能看但点不动"）。 */
function ServiceWorkerRegister() {
  if (process.env.NODE_ENV !== "production") return null;
  return (
    <script
      dangerouslySetInnerHTML={{
        __html: `if('serviceWorker' in navigator){window.addEventListener('load',function(){
if(window.__fluxSwUpdated)return;
navigator.serviceWorker.register('/sw.js').then(function(reg){
reg.addEventListener('updatefound',function(){
var nw=reg.installing;
if(!nw)return;
nw.addEventListener('statechange',function(){
if(nw.state==='installed'&&navigator.serviceWorker.controller){
window.__fluxSwUpdated=true;
nw.postMessage('SKIP_WAITING');
}
});
});
navigator.serviceWorker.addEventListener('message',function(e){
if(e.data==='FLUX_SW_ACTIVATED'&&window.__fluxSwUpdated){
window.__fluxSwUpdated=false;location.reload();
}
});
}).catch(function(){/* SW 失败不阻塞站点 */});
})}`,
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
