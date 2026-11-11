import type { Metadata, Viewport } from "next";
import { Suspense } from "react";
import "./globals.css";
import { getAllTerms, getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { glassStyle } from "@/lib/site-glass";
import { siteBase } from "@/lib/site-url";
import { LocaleProvider } from "@/i18n/client";
import { PwaInstallBar } from "@/components/pwa-install-bar";
import { ToastProvider } from "@/components/ui/toast-provider";
import { SkipLink } from "@/components/ui/skip-link";
import { SparkBridge } from "@/components/spark-bridge";
import {
  THEMES,
  THEME_IDS,
  THEME_STORAGE_KEY,
  themeCssManifest,
  themeIdByScheme,
} from "@/lib/themes";

/** 移动端方案 M1：viewport-fit=cover 让 env(safe-area-inset-*) 生效——
 *  layout.tsx 底 Tab/操作条的 safe-area 内边距此前在 iOS 全面屏完全不工作。 */
export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  themeColor: [
    // design-cov-exempt: meta theme-color 不吃 CSS 变量，只能字面量
    { media: "(prefers-color-scheme: light)", color: "#f5faff" },
    // design-cov-exempt: 同上（与 --surface-page 明暗两值同源）
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
    // 站点指纹：第三方工具（如 auto_feed 适配层）据此识别 FluxTorrent
    generator: "FluxTorrent",
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
      // 默认分享卡（内置站标 + 横版字标合成）；站长可在后台改文案但卡片暂固定
      images: [
        { url: "/brand/og.png", width: 1200, height: 630, alt: brandName },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title: brandName,
      description: desc,
      images: ["/brand/og.png"],
    },
    // manifest 由 app/manifest.ts 运行时生成（品牌跟随 site_profile.brand），Next 自动注入 <link rel="manifest">
    // 站点图标（0214）：site_favicon 有值优先（支持站内附件 /api/v1/attachments/{sha}）
    icons: profile.site_favicon?.trim()
      ? { icon: profile.site_favicon.trim(), apple: profile.site_favicon.trim() }
      : {
          icon: "/icons/icon-192.png",
          apple: "/icons/apple-touch-icon.png",
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
 *  同时同步 PWA theme-color，并监听系统偏好变化（未手动选择时实时跟随）。
 *  W-42：chrome 色表与档位解析搬到 lib/themes.ts 注册表（唯一真源），
 *  本脚本序列化注册表下发（保持内联、零水合依赖）；"auto" 档存库值
 *  在首绘即解析为具体主题——CSS 与 meta theme-color 都不认识 "auto"；
 *  明暗族按 ThemeDef.scheme 查（themeIdByScheme），不靠注册表位置。
 *
 *  2026-11-11 瘦身批：山海经字体/主题 CSS 移出全局链后，apply(t) 里同步
 *  按需注入/移除 <link>（ES5 等价于 lib/themes.ts 的 syncThemeCss；URL
 *  清单从构建期 manifest 序列化进来）。同步 appendChild 的 <link> 加载
 *  完成前该主题首帧会有短暂字体回落（Ma Shan Zheng → 楷体栈）——只影响
 *  山海经用户自己的刷新，默认主题用户零感知、零下载。 */
function ThemeNoFlash() {
  const CH = JSON.stringify(
    Object.fromEntries(
      Object.entries(THEMES).map(([k, v]) => [k, v.chrome]),
    ),
  );
  const ids = JSON.stringify(THEME_IDS);
  // 明暗族 → 主题 id（scheme 字段唯一真源，不做 IDS[0]/IDS[1] 位置假设）
  const SC = JSON.stringify({
    light: themeIdByScheme("light"),
    dark: themeIdByScheme("dark"),
  });
  // 山海经 CSS 的注入清单（[themeId 前缀, fontsUrl, themeUrl]；同步
  // scripts/gen-theme-css.mjs 的 manifest 键）
  const SHJ = JSON.stringify([
    "shanhaijing",
    themeCssManifest.fonts,
    themeCssManifest.theme,
  ]);
  const script =
    `(function(){var CH=${CH},IDS=${ids},SC=${SC},SHJ=${SHJ};` +
    `function chrome(t){var m=document.querySelector('meta[name="theme-color"]');` +
    `if(!m){m=document.createElement('meta');m.name='theme-color';document.head.appendChild(m);}m.content=CH[t];}` +
    `function css(t){var want=(t&&t.indexOf(SHJ[0])===0)?[SHJ[1],SHJ[2]]:[];` +
    `var ls=document.querySelectorAll('link[data-theme-css]');` +
    `for(var i=0;i<want.length;i++){var ok=false;` +
    `for(var j=0;j<ls.length;j++){var h=ls[j].getAttribute('href');` +
    `if(h===want[i]){ok=true;break;}}` +
    `if(!ok){var l=document.createElement('link');l.rel='stylesheet';` +
    `l.href=want[i];l.setAttribute('data-theme-css','');` +
    `document.head.appendChild(l);}}` +
    `for(var k=ls.length-1;k>=0;k--){var hh=ls[k].getAttribute('href');` +
    `if(want.indexOf(hh)<0)ls[k].parentNode.removeChild(ls[k]);}}` +
    `function apply(t){document.documentElement.dataset.theme=t;` +
    `chrome(t);css(t);}` +
    `function sys(){return window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches?SC.dark:SC.light;}` +
    `function pick(v){return (v==='auto'||IDS.indexOf(v)<0)?sys():v;}` +
    `try{var saved=localStorage.getItem('${THEME_STORAGE_KEY}');` +
    `apply(pick(saved));` +
    `if(!saved||saved==='auto'){` +
    `var mq=window.matchMedia('(prefers-color-scheme: dark)');` +
    `if(mq.addEventListener){mq.addEventListener('change',function(){` +
    `try{var cur=localStorage.getItem('${THEME_STORAGE_KEY}');` +
    `if(!cur||cur==='auto'){apply(sys());}}catch(_){}});}}` +
    `}catch(e){}})();`;
  return <script dangerouslySetInnerHTML={{ __html: script }} />;
}

/** 旧内核运行时兜底（2026-10-04 生产站实测）：Next.js 15 预编译的路由器
 *  内部用了 Array.prototype.at()（Chrome 92+ 才支持），且这段 runtime 不经过
 *  SWC 转译、next 自带 polyfill chunk 也不含 .at —— 旧内核（实测 Chrome 57-91，
 *  生产访问日志 128+ 次）水合必抛 TypeError，整站落 error.tsx「页面出错了」。
 *  这里在水合前的同步内联脚本里补 .at()（MDN 标准 polyfill 改写，ES5 语法）。
 *  有 .at 的内核一行不执行，零开销。 */
function LegacyRuntimePolyfill() {
  const script =
    `(function(){function at(n){n=Math.trunc(n)||0;` +
    `if(n<0)n+=this.length;if(n<0||n>=this.length)` +
    `return undefined;return this[n];}` +
    `if(!Array.prototype.at)Array.prototype.at=at;` +
    `if(!String.prototype.at)String.prototype.at=at;})();`;
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
        // 键名去前缀是 glow，而全站 CSS 读的是 --compat-glow（base.css / pages.css /
        // theme-tide.css）——直注 --glow 没有任何消费方，后台改辉光色不生效。
        return `--compat-glow:${v};--compat-glow-veil:${v}1a;`;
      }
      return `--${varName}:${v};`;
    })
    .join("");
  return <style dangerouslySetInnerHTML={{ __html: `:root{${css}}` }} />;
}

export default async function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  const { locale, currency } = await getDict();
  // 术语规则 prop 下发（TermRule 仅 canonical/replacement 两字段，hxpt 实测
  // 0 条）：i18n/dict-impl.ts 用同一 applyTerms 改写懒加载的原始字典，与 RSC
  // 出口的 getDict 同函数同输入 → 水合前后逐字一致。
  // 第二轮瘦身的核心：字典不再作为 LocaleProvider 的 prop 进入 flight
  // payload（曾按 174KB/页重复计费且 HTML 不可缓存），改为每语言一颗
  // 懒加载 chunk（SW cache-first，服务端模块初始化预热、SSR 零挂起）。
  // Suspense 只在字典未就绪时接住挂起，fallback 不写内容——预热使 promise
  // 在抛出前已落定，React 同步重试，绝不显示空白。
  const terms = await getAllTerms();
  const profile = await getSiteProfile();
  // 板块磨砂玻璃（0448）：作用域挂在 <html> 上——顶栏搜索框与部分弹层是
  // portal 到 body 的，挂在 .site-shell 上会漏掉它们（见 glass.css 注释）。
  const glass = glassStyle(profile?.glass);
  return (
    <html
      lang={locale}
      suppressHydrationWarning
      data-glass={glass ? "on" : undefined}
      style={glass ?? undefined}
    >
      <head>
        <ThemeTokenStyle tokens={profile.theme_tokens} />
        {/* TIDE 字体（P5）：已改为**自托管**（见 app/styles/fonts-tide.css + public/fonts/），
            不再请求 Google Fonts —— 保留 unicode-range 分片，浏览器按需加载，
            首访体积不变但彻底摆脱外网依赖；回退链（Songti SC / SimSun）仍是衬线。 */}
      </head>
      <body>
        {/* 旧内核 polyfill 必须最先执行（先于 next 的路由器水合；见组件注释） */}
        <LegacyRuntimePolyfill />
        <ThemeNoFlash />
        <Suspense fallback={null}>
          <LocaleProvider locale={locale} currency={currency} terms={terms}>
            {/* 设计系统 W-01/W-04：SkipLink 是 Tab 第一站；ToastProvider
                挂载一次，全站写操作反馈走 toast 协议（role=status/alert） */}
            <SkipLink />
            {/* 余额广播的 SSR 侧消费者（写魔力后广播一次，重取直出的数字） */}
            <SparkBridge />
            <ToastProvider>
              <PwaInstallBar />
              {children}
              <ServiceWorkerRegister />
            </ToastProvider>
          </LocaleProvider>
        </Suspense>
      </body>
    </html>
  );
}
