import { Header, MobileTabBar } from "@/components/layout";
import { buildNav } from "@/lib/nav-menu";
import { getMenuItems } from "@/lib/data";
import { getSiteProfile, getSiteDims } from "@/lib/site-profile";
import { Footer } from "@/components/footer";
import { MotionShell } from "@/components/motion";
import { ScrollButtons } from "@/components/scroll-buttons";
import { getDict } from "@/i18n/server";

/**
 * 站内布局：桌面顶栏 + 移动底部 Tab + 内容容器 + 页脚。
 * 认证类页面（/login /register /forgot /reset /offline）使用各自的纯净布局，不挂导航。
 */
export default async function MainLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  // 布局四路并发（2026-10-18 主菜单切换提速）：此前 dict→profile→dims→menu
  // 四连串行，每个导航 RSC 都白等 3 个 API RTT。getSiteProfile/getSiteDims
  // 是 React cache 同请求去重，页面侧同名调用不产生第二次请求。
  const [{ dict, currency }, profile, dims, customItems] = await Promise.all([
    getDict(),
    getSiteProfile().catch(() => null),
    getSiteDims(),
    getMenuItems("topbar").catch(() => []),
  ]);
  // buildNav 此处仅为类型兼容保留调用链（Header 内部会再算一次，
  // getSiteProfile/getDict 是 React cache 同请求去重，无二次请求）。
  buildNav({
    nav: dict.nav,
    tabbar: dict.tabbar,
    currency,
    modules: (k) => (profile?.modules?.[k] ?? true) !== false,
    dims: (k) => dims.includes(k),
    customItems,
  });
  return (
    <>
      <Header />
      {/* 动效壳（P4）：无 DOM 输出，只做顶栏吸顶态 + 滚动进场（渐进增强，不隐藏 SSR 内容） */}
      <MotionShell />
      {/* 2026-10-19 平板对齐手机：M4 细侧栏（tsidebar）退役——平板区间
          改由底部 5 Tab + 顶栏汉堡抽屉承担导航（与手机同构），
          侧栏让出内容宽度。桌面 ≥lg 仍是顶栏平铺。 */}
      {/* 内容容器与页头/页脚同宽（1536）：此前内容 1280、页头页脚 1536，
          1920 屏上内容比页头页脚窄一截，显得小气（2026-10-20 用户口径） */}
      <div className="flex">
        <main
          id="main-content"
          className="mx-auto min-w-0 w-full max-w-[1536px] flex-1 px-4 pb-24
            pt-6 md:px-6"
        >
          {children}
        </main>
      </div>
      <Footer />
      {/* 右下角「至顶端 / 至底端」：滚动超过 240px 出现，移动端抬离底部 TabBar */}
      <ScrollButtons />
      <MobileTabBar />
    </>
  );
}
