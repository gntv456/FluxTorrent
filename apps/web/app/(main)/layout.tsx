import { Header, MobileTabBar } from "@/components/layout";
import { TabletSidebar } from "@/components/tablet-sidebar";
import { buildNav } from "@/lib/nav-menu";
import { getMenuItems } from "@/lib/data";
import { getSiteProfile } from "@/lib/site-profile";
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
  const { dict, currency } = await getDict(); // 布局语言与页面同源
  // M4 平板细侧栏：与顶栏同源配置（lib/nav-menu.ts）
  const profile = await getSiteProfile().catch(() => null);
  const customItems = await getMenuItems("topbar").catch(() => []);
  const { primary: navPrimary } = buildNav({
    nav: dict.nav,
    tabbar: dict.tabbar,
    currency,
    modules: (k) => (profile?.modules?.[k] ?? true) !== false,
    customItems,
  });
  return (
    <>
      <Header />
      {/* 动效壳（P4）：无 DOM 输出，只做顶栏吸顶态 + 滚动进场（渐进增强，不隐藏 SSR 内容） */}
      <MotionShell />
      {/* M4：md-lg 平板细侧栏（底 Tab 同步隐藏，见 CSS） */}
      <div className="flex">
        <TabletSidebar
          items={navPrimary.map((n) => ({ ...n, icon: null }))}
          ariaLabel={dict.nav.ariaPrimary}
        />
        <main
          className="mx-auto min-w-0 w-full max-w-[1280px] flex-1 px-4 pb-24
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
