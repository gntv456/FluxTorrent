import { Header, MobileTabBar } from "@/components/layout";
import { TabletSidebar } from "@/components/tablet-sidebar";
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
  const { primary: navPrimary } = buildNav({
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
      {/* M4：md-lg 平板细侧栏（底 Tab 同步隐藏，见 CSS） */}
      {/* 内容容器与页头/页脚同宽（1536）：此前内容 1280、页头页脚 1536，
          1920 屏上内容比页头页脚窄一截，显得小气（2026-10-20 用户口径） */}
      <div className="flex">
        <TabletSidebar
          items={navPrimary.map((n) => ({ ...n, icon: null }))}
          ariaLabel={dict.nav.ariaPrimary}
        />
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
