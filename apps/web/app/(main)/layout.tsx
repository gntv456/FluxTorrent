import { Header, MobileTabBar } from "@/components/layout";
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
  await getDict(); // 布局语言与页面同源（Header 内部也各自 getDict）
  return (
    <>
      <Header />
      {/* 动效壳（P4）：无 DOM 输出，只做顶栏吸顶态 + 滚动进场（渐进增强，不隐藏 SSR 内容） */}
      <MotionShell />
      <main className="mx-auto w-full max-w-[1280px] px-4 pb-24 pt-6 md:px-6">
        {children}
      </main>
      <Footer />
      {/* 右下角「至顶端 / 至底端」：滚动超过 240px 出现，移动端抬离底部 TabBar */}
      <ScrollButtons />
      <MobileTabBar />
    </>
  );
}
