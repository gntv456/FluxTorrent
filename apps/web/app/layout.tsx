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
import { readDictChunk } from "@/lib/dict-chunks";
import { ThemeNoFlash } from "@/lib/theme-no-flash";
import { DensityNoFlash } from "@/lib/density-no-flash";
import { UxMetricsProbe } from "@/components/ux-metrics-probe";

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
    // 站点图标（0214）：site_favicon 有值优先（支持站内附件 /api/v1/attachments/{sha}）。
    // 原 app/favicon.ico 约定文件已挪 public/（2026-11-20 好学站 favicon 修复）：
    // 约定文件会让 Next 无条件注入 <link rel="icon" href="/favicon.ico">，站点
    // site_favicon 误指它时浏览器合并结果恒为内置 FT 默认图，配置链被兜底架空。
    // 挪出后图标完全由本配置驱动；public/favicon.ico 仅保留 URL 作直连兜底。
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
  // 当前语言的字典 chunk URL（postbuild 清单；缺失时 undefined → 不注入）
  const dictChunkHref = readDictChunk(locale);
  return (
    <html
      lang={locale}
      suppressHydrationWarning
      data-glass={glass ? "on" : undefined}
      style={glass ?? undefined}
    >
      <head>
        <ThemeTokenStyle tokens={profile.theme_tokens} />
        {/* 字典懒 chunk 提前发现（2026-11-11 提速批 #5）：i18n/dict-impl 的
            import() 要等 eager JS 执行到 Provider 挂起才发起——劣化链路上
            串行多一跳 RTT。postbuild 的 gen-dict-chunks.mjs 落清单，SSR 在
            head 注入 preload 让字典与 eager JS 并行下载。清单缺失（本地
            dev/旧构建）静默跳过，行为退化为现状。 */}
        {dictChunkHref && (
          <link rel="preload" href={dictChunkHref} as="script" />
        )}
        {/* TIDE 字体（P5）：已改为**自托管**（见 app/styles/fonts-tide.css + public/fonts/），
            不再请求 Google Fonts —— 保留 unicode-range 分片，浏览器按需加载，
            首访体积不变但彻底摆脱外网依赖；回退链（Songti SC / SimSun）仍是衬线。 */}
      </head>
      <body>
        {/* 旧内核 polyfill 必须最先执行（先于 next 的路由器水合；见组件注释） */}
        <LegacyRuntimePolyfill />
        <ThemeNoFlash />
        <DensityNoFlash />
        <Suspense fallback={null}>
          <LocaleProvider locale={locale} currency={currency} terms={terms}>
            {/* 设计系统 W-01/W-04：SkipLink 是 Tab 第一站；ToastProvider
                挂载一次，全站写操作反馈走 toast 协议（role=status/alert） */}
            <SkipLink />
            {/* 余额广播的 SSR 侧消费者（写魔力后广播一次，重取直出的数字） */}
            <SparkBridge />
            <ToastProvider>
              <UxMetricsProbe />
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
