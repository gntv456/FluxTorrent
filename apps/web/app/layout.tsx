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

/** 主题 no-flash：首绘前读 localStorage（缺省跟随系统 prefers-color-scheme）
 *  并写入 <html data-theme>，避免夜间用户刷新时白屏闪烁；
 *  同时同步 PWA theme-color，并监听系统偏好变化（未手动选择时实时跟随）。 */
function ThemeNoFlash() {
  const script =
    `(function(){var CH={baozi:'#f5faff','baozi-night':'#0f1424'};` +
    `function chrome(t){var m=document.querySelector('meta[name="theme-color"]');` +
    `if(!m){m=document.createElement('meta');m.name='theme-color';document.head.appendChild(m);}m.content=CH[t];}` +
    `function apply(t){document.documentElement.dataset.theme=t;chrome(t);}` +
    `try{var t=localStorage.getItem('flux-theme');` +
    `if(t!=='baozi'&&t!=='baozi-night'){` +
    `t=window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches?'baozi-night':'baozi';` +
    `var mq=window.matchMedia('(prefers-color-scheme: dark)');` +
    `if(mq.addEventListener){mq.addEventListener('change',function(e){` +
    `try{if(!localStorage.getItem('flux-theme')){apply(e.matches?'baozi-night':'baozi');}}catch(_){}});}}` +
    `apply(t);}catch(e){}})();`;
  return <script dangerouslySetInnerHTML={{ __html: script }} />;
}

export default async function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  const { dict, locale } = await getDict();
  return (
    <html lang={locale} suppressHydrationWarning>
      <head>
        {/* Seedlight 展示字体：站酷快乐体（仅 H1/品牌/等级名，小面积使用）。
            preconnect + display=swap：字体未就绪时标题先以回退栈渲染，不阻塞首屏。 */}
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link
          rel="stylesheet"
          href="https://fonts.googleapis.com/css2?family=ZCOOL+KuaiLe&display=swap"
        />
      </head>
      <body>
        <ThemeNoFlash />
        <LocaleProvider dict={dict} locale={locale}>
          {children}
          <ServiceWorkerRegister />
        </LocaleProvider>
      </body>
    </html>
  );
}
