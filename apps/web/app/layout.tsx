import type { Metadata, Viewport } from "next";
import "./globals.css";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { siteBase } from "@/lib/site-url";
import { LocaleProvider } from "@/i18n/client";

/** 移动端方案 M1：viewport-fit=cover 让 env(safe-area-inset-*) 生效——
 *  layout.tsx 底 Tab/操作条的 safe-area 内边距此前在 iOS 全面屏完全不工作。 */
export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f5faff" },
    { media: "(prefers-color-scheme: dark)", color: "#0f1424" },
  ],
};

export async function generateMetadata(): Promise<Metadata> {
  const { dict } = await getDict();
  // 品牌名跟随站点设定（site_name，站长后台可改）；默认 FluxTorrent
  const profile = await getSiteProfile();
  const brandName = profile.brand || "FluxTorrent";
  // SEO（0201）：META 描述/关键词与收录开关真的接进来了——此前设置页那排键
  // 只有 metadescription 被 RSS 用，前台一律用字典默认，站长改完不生效。
  const seo = profile.seo ?? {};
  // 描述三级回落：站长填的 META 描述 → 站点简介 → 自带词表。自带文案写着
  // 「通用 PT 建站系统」，与「不偏向任何 PT 类型」的定位相反，所以站长自己的
  // 话必须排在它前面。
  const desc =
    seo.description?.trim() ||
    profile.site_desc?.trim() ||
    dict.meta.description;
  const base = siteBase();
  const keywords = (seo.keywords ?? "")
    .split(/[,，]/)
    .map((s) => s.trim())
    .filter(Boolean);
  return {
    ...(base ? { metadataBase: base } : {}),
    title: `${brandName} · ${dict.meta.titleSuffix}`,
    description: desc,
    ...(keywords.length ? { keywords } : {}),
    // 未显式允许收录 ⇒ 私有站默认对爬虫关门（robots.txt 同步 Disallow）
    robots: seo.indexable
      ? { index: true, follow: true }
      : { index: false, follow: false },
    openGraph: {
      type: "website",
      siteName: brandName,
      title: brandName,
      description: desc,
      ...(base ? { url: base.href } : {}),
    },
    twitter: { card: "summary", title: brandName, description: desc },
    // manifest 由 app/manifest.ts 运行时生成（品牌跟随 site_profile.brand），Next 自动注入 <link rel="manifest">
    // 站点图标（0214）：site_favicon 有值优先（支持站内附件 /api/v1/attachments/{sha}）
    icons: profile.site_favicon?.trim()
      ? { icon: profile.site_favicon.trim(), apple: profile.site_favicon.trim() }
      : {
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

/** 主题令牌覆盖（0189 R4.6）：站长在后台设置的品牌色注入 :root。
 *  仅接受 #rrggbb（后端已过滤，此处再守一道）；空 = 不注入，用默认 Aurora。 */
function ThemeTokenStyle({ tokens }: { tokens?: Record<string, string> }) {
  const entries = Object.entries(tokens ?? {}).filter(
    ([k, v]) =>
      k.startsWith("theme_token_") && /^#[0-9a-fA-F]{6}$/.test(v ?? ""),
  );
  if (!entries.length) return null;
  const css = entries
    .map(([k, v]) => {
      const varName = k.replace("theme_token_", "");
      if (varName === "ribbon") {
        // 彩带底色：纯色替换渐变
        return `--ribbon:${v};--grad-rainbow:${v};`;
      }
      if (varName === "glow") {
        // 键名去前缀是 glow，而全站 CSS 读的是 --brand-glow（base.css / pages.css /
        // theme-tide.css）——直注 --glow 没有任何消费方，后台改辉光色不生效。
        return `--brand-glow:${v};--brand-glow-veil:${v}1a;`;
      }
      return `--${varName}:${v};`;
    })
    .join("");
  return <style dangerouslySetInnerHTML={{ __html: `:root{${css}}` }} />;
}

export default async function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  const { dict, locale, currency } = await getDict();
  const profile = await getSiteProfile();
  return (
    <html lang={locale} suppressHydrationWarning>
      <head>
        <ThemeTokenStyle tokens={profile.theme_tokens} />
        {/* TIDE 字体（P5）：已改为**自托管**（见 app/styles/fonts-tide.css + public/fonts/），
            不再请求 Google Fonts —— 保留 unicode-range 分片，浏览器按需加载，
            首访体积不变但彻底摆脱外网依赖；回退链（Songti SC / SimSun）仍是衬线。 */}
      </head>
      <body>
        <ThemeNoFlash />
        <LocaleProvider dict={dict} locale={locale} currency={currency}>
          {children}
          <ServiceWorkerRegister />
        </LocaleProvider>
      </body>
    </html>
  );
}
