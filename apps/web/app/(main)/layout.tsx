import { Header, MobileTabBar } from "@/components/layout";
import { buildNav } from "@/lib/nav-menu";
import { getMenuItems } from "@/lib/data";
import { getSiteProfile, getSiteDims } from "@/lib/site-profile";
import { backgroundOpacity, cssBackgroundImage } from "@/lib/site-background";
import { Footer } from "@/components/footer";
import { MotionShell } from "@/components/motion";
import { ScrollButtons } from "@/components/scroll-buttons";
import { ShjAtlas } from "@/components/shj-atlas";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { requireNotMaintaining } from "@/lib/maintenance";

/**
 * 站内布局：桌面顶栏 + 移动底部 Tab + 内容容器 + 页脚。
 * 认证类页面（/login /register /forgot /reset /offline）使用各自的纯净布局，不挂导航。
 * 维护模式（0462）：maintenance_mode=yes 时非 staff 302 /maintenance；
 * staff 穿透并在顶部看维护横幅（随时回后台关闭）。
 */
export default async function MainLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  // 维护闸门先于一切取数（维护中非 staff 不再付四次并发请求的 RTT）
  const maintaining = await requireNotMaintaining();
  // 布局四路并发（2026-10-18 主菜单切换提速）：此前 dict→profile→dims→menu
  // 四连串行，每个导航 RSC 都白等 3 个 API RTT。getSiteProfile/getSiteDims
  // 是 React cache 同请求去重，页面侧同名调用不产生第二次请求。
  const [{ dict, currency }, profile, dims, customItems] = await Promise.all([
    getDict(),
    getSiteProfile().catch(() => null),
    getSiteDims(),
    getMenuItems("topbar").catch(() => []),
  ]);
  // 山海经图鉴侧栏（乙）与 Header 共用同一份 buildNav 产物；
  // getSiteProfile/getDict 是 React cache 同请求去重，无二次请求。
  const shjNav = buildNav({
    nav: dict.nav,
    tabbar: dict.tabbar,
    currency,
    modules: (k) => (profile?.modules?.[k] ?? true) !== false,
    dims: (k) => dims.includes(k),
    customItems,
    libraryCats: (profile?.categories ?? [])
      .filter((c) => !c.parent_id)
      .map((c) => ({ id: c.id, name: c.name })),
  });
  // 侧栏境界签用户摘要：与 Header 同一个 /me/overview（Next 请求记忆化，
  // 同一渲染周期内相同 GET 只发一次，不产生第二个 RTT）；失败回落 null
  const shjOv = await api
    .get<{ username?: string; class_name?: string; uploaded?: number }>(
      "/api/v1/me/overview",
    )
    .catch(() => null);
  const shjMe = shjOv?.username
    ? {
        username: shjOv.username,
        className: shjOv.class_name ?? "",
        uploaded: `${Math.round((shjOv.uploaded ?? 0) / 1e9)}G`,
      }
    : null;
  return (
    <>
      {/* 站点背景图（0445）：固定底层（z=0），内容壳 z=10 压在上面；
          空 URL 不渲染。负 z-index 会被 body 的不透明底盖住，所以用 0/10 分层。 */}
      {(() => {
        const bg = cssBackgroundImage(profile?.backgrounds?.site?.url);
        if (!bg) return null;
        return (
          <div
            className="site-backdrop"
            aria-hidden
            style={{
              backgroundImage: bg,
              opacity:
                backgroundOpacity(
                  profile?.backgrounds?.site?.opacity,
                  25,
                ) / 100,
            }}
          />
        );
      })()}
      {/* 山海经图鉴侧栏：仅 [data-theme="shanhaijing"] 下显示（其余主题
          display:none 零影响）；fixed 定位，DOM 序在内容前保证 Tab 先导航。
          默认渲染不增请求（nav 数据与本布局同源、摘要与 Header 请求记忆化去重） */}
      <ShjAtlas
        primary={shjNav.primary}
        groups={shjNav.groups}
        summary={shjMe}
        brand={profile?.brand || dict.common.brand}
      />
      <div className="site-shell">
        {/* 维护横幅（0462）：只有 staff 会走到这里（非 staff 已被闸门
            302 走）——提醒「站现在对外是维护态」，别当普通通知弹 */}
        {maintaining ? (
          <div
            role="status"
            className="bg-[var(--amber,#b45309)]/12 px-4 py-2 text-center
              text-sm text-[var(--amber,#b45309)]"
          >
            {dict.maintenance.banner}
          </div>
        ) : null}
        <Header />
        {/* 动效壳（P4）：无 DOM 输出，只做顶栏吸顶态 + 滚动进场（渐进增强，不隐藏 SSR 内容） */}
        <MotionShell />
        {/* 2026-10-19 平板对齐手机：M4 细侧栏（tsidebar）退役——平板区间
            改由底部 5 Tab + 顶栏汉堡抽屉承担导航（与手机同构），
            侧栏让出内容宽度。桌面 ≥lg 仍是顶栏平铺。 */}
        {/* 内容容器与页头/页脚同宽（1760）：此前内容 1280、页头页脚 1536，
            1920 屏上内容比页头页脚窄一截（2026-10-20 用户口径）；2026-11-02
            桌面端再放宽 1536→1760——更接近整页宽度，2560+ 屏仍留边不贴边。 */}
        <div className="flex">
          <main
            id="main-content"
            className="mx-auto min-w-0 w-full max-w-[1760px] flex-1 px-4
              pb-24 pt-6 md:px-6"
          >
            {children}
          </main>
        </div>
        <Footer />
        {/* 右下角「至顶端 / 至底端」：滚动超过 240px 出现，移动端抬离底部 TabBar */}
        <ScrollButtons />
        <MobileTabBar />
      </div>
    </>
  );
}
