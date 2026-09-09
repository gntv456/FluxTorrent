import { Header, MobileTabBar } from "@/components/layout";
import { getDict } from "@/i18n/server";

/**
 * 站内布局：桌面顶栏 + 移动底部 Tab + 内容容器。
 * 认证类页面（/login /register /forgot /reset /offline）使用各自的纯净布局，不挂导航。
 */
export default async function MainLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  await getDict(); // 布局语言与页面同源（Header 内部也各自 getDict）
  return (
    <>
      <Header />
      <main className="mx-auto w-full max-w-[1280px] px-4 pb-24 pt-6 md:px-6">
        {children}
      </main>
      <MobileTabBar />
    </>
  );
}
