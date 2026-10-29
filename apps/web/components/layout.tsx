import Link from "next/link";
import { Suspense } from "react";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { getSiteProfile, getSiteDims } from "@/lib/site-profile";
import { LocaleSwitcher } from "@/components/locale-switcher";
import { ExamCapsule } from "@/components/exam-capsule";
import { ThemeToggle } from "@/components/theme-toggle";
import { UserMenu } from "@/components/user-menu";
import { MainMenu } from "@/components/main-menu";
import { CustomMenu } from "@/components/custom-menu";
import { Icon } from "@/components/icons";
import { getMenuItems } from "@/lib/data";
import { buildNav } from "@/lib/nav-menu";
import { MobileNavShell } from "@/components/mobile-nav-shell";
import { GlobalSearch } from "@/components/global-search";
import { TabLink } from "@/components/tab-link";
import { api } from "@/lib/api-client";

/**
 * Seedlight 页头：单行导航条（logo 居左 + 一级/更多菜单居中 + 主题/语言/头像弹窗居右）。
 * 用户栏（userbar）已收进头像弹窗（0147，同类站 同类站布局口径）——导航条整条吸顶。
 */
export async function Header() {
  const { dict, locale, currency } = await getDict();
  const profile = await getSiteProfile();
  const dims = await getSiteDims();
  /** 模块开关（U1 §6.2）：缺键视为开（T3 缺省=现状），与 API 侧 default_on 口径一致 */
  const mod = (k: string) => profile.modules[k] !== false;
  const brand = profile.brand || dict.common.brand;
  // 自定义菜单（location=topbar，nav.custom_enabled 开启时接口才返回非空）
  const customItems = await getMenuItems("topbar");
  // 抽屉用户摘要（M2）：失败回落游客文案，不阻塞页头
  const ov = await api
    .get<{
      username?: string;
      class_name?: string;
      uploaded?: number;
      spark_balance?: number;
    }>("/api/v1/me/overview")
    .catch(() => null);
  const me = ov?.username
    ? {
        username: ov.username,
        className: ov.class_name ?? "",
        uploaded: `${Math.round((ov.uploaded ?? 0) / 1e9)}G`,
        spark: (ov.spark_balance ?? 0).toLocaleString(dateLocale(locale)),
      }
    : null;
  // 导航配置单源（M2）：桌面/抽屉/底 Tab「更多」共用 lib/nav-menu.ts
  const { primary, groups } = buildNav({
    nav: dict.nav,
    tabbar: dict.tabbar,
    currency,
    modules: mod,
    dims: (k) => dims.includes(k),
    customItems,
  });

  return (
    <>
      <header className="tide-header border-b border-line">
        {/* 单行导航条：品牌（logo）居左 + 一级/更多菜单 + 主题/语言/头像弹窗居右 */}
        <div className="mx-auto flex min-h-[56px] w-full max-w-[1536px] items-center gap-3 px-3 py-1 md:min-h-[72px] md:gap-5 md:px-6 md:py-2">
          {/* M1：品牌长站名在 375px 会把右侧工具区推出视口（brand + 工具 + 头像
              总宽 >375）。窄屏截断品牌字，min-w-0 允许收缩；完整名留在 logo title。
              ⚠️ min-w-0 必须挂在 Link 上：truncate 的 span 是 nowrap 文本，
              flex 自动最小尺寸=min-content（整段站名），Link overflow 可见时
              min-w:auto 顶住不缩——实测 390px 溢出 13px（M5.1 复核 2026-10-15）。 */}
          <Link
            href="/"
            title={brand}
            className="flex min-w-0 max-w-[56%] shrink items-center
              gap-2 md:max-w-none"
          >
            {/* 品牌标记：内置 FluxTorrent 站标（白字深底，浅色/夜间均可见）；
                站点自定义 logo 走 site_settings.site_logo（登录页品牌图） */}
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img
              src="/brand/logo-mark.png"
              alt=""
              width={30}
              height={30}
              className="shrink-0"
            />
            <span className="truncate font-display text-2xl text-ink">
              {brand}
            </span>
          </Link>
          <div className="hidden min-w-0 flex-1 lg:block">
            {/* Suspense：MainMenu 用 useSearchParams 区分带参条目（官种），
                预渲染路由要求它包在边界里。2026-10-21：<lg 不再渲染「更多 ▾」
                ——抽屉已承载全部同源入口，两者并排属于重复导航，也白占顶栏宽度。 */}
            <Suspense fallback={null}>
              <MainMenu
                items={primary}
                groups={groups}
                moreLabel={`${dict.nav.more} ▾`}
                ariaLabel={dict.nav.ariaPrimary}
              />
            </Suspense>
          </div>
          {/* IA-1（W-30）：顶栏搜索直显框——/top 从一级让位给它。
              <md 收进汉堡抽屉（右工具区在 375px 已超宽），桌面常驻。 */}
          <div className="hidden w-44 shrink-0 md:block lg:w-56">
            <GlobalSearch
              placeholder={dict.common.globalSearchPh}
              tipTorrents={dict.common.gsTorrents}
              tipTopics={dict.common.gsTopics}
              tipEmpty={dict.common.gsEmpty}
              tipMore={dict.common.gsMore}
              tipSearching={dict.common.gsSearching}
              tipError={dict.common.gsError}
              retryLabel={dict.common.gsRetry}
            />
          </div>
          <div className="ml-auto flex shrink-0 items-center gap-2">
            {/* 移动/平板汉堡（<lg）：抽屉与桌面同源配置；摘要行回落游客。
                2026-10-19 平板收敛：768–1023 也收进抽屉（实测该宽度段
                一级 5 项竖排 5 行、顶栏 259px），lg+ 才恢复桌面平铺。 */}
            <div className="lg:hidden">
              <MobileNavShell primary={primary} groups={groups} summary={me} />
            </div>
            {/* 主题切换 <md 收进汉堡抽屉：280px 折叠屏外屏态右工具区
                （汉堡+主题+头像）会挤出横向滚动 */}
            <div className="hidden md:block">
              <ThemeToggle label={dict.common.themeToggle} />
            </div>
            {/* 语言切换器（0209 P2-17）：site_settings.locale_switcher_enabled=no 隐藏（单语站）。
                M2：<md 隐藏——右工具区在 375px 超宽 ~39px，
                语言是低频操作，移入抽屉外的登录页/页脚仍可达。 */}
            {profile.locale_switcher_enabled !== "no" && (
              <div className="hidden md:block">
                <LocaleSwitcher current={locale} />
              </div>
            )}
            {/* 0427 考核进度胶囊（同类站 UI 对标采纳③）：onboard/periodic
                考核进行中时常驻；无考核不渲染。md+ 显示，窄屏收抽屉摘要。
                exams 模块关闭时 /me/exams 回 4101，挂载了就是每页一次
                必然失败的请求——与导航同一把模块闸。 */}
            {mod("exams") && <ExamCapsule />}
            {/* 头像弹窗（0147）：点击头像展开用户下拉（摘要/数据/快捷/信箱） */}
            <UserMenu loginLabel={dict.common.login} />
          </div>
        </div>
      </header>
      {/* 自定义菜单（location=topbar）已在上方替换主菜单一级项，无独立行 */}
    </>
  );
}

/** 移动/平板底部 5 Tab（Seedlight §3）：首页/发现/发布（极光凸起）/消息/我的。
 *  2026-10-19 平板对齐手机：<lg（含 768–1023 平板）同样显示——顶栏一级
 *  已收进汉堡抽屉，底部 Tab 提供高频四口，与手机完全同构。 */
export async function MobileTabBar() {
  const { dict } = await getDict();
  const profile = await getSiteProfile();
  const mod = (k: string) => profile.modules[k] !== false;
  const tabs = [
    {
      href: "/",
      label: dict.tabbar.home,
      icon: <Icon name="home" size={23} />,
    },
    {
      href: "/torrents",
      label: dict.tabbar.search,
      icon: <Icon name="search" size={23} />,
    },
    {
      href: "/upload",
      label: dict.tabbar.publish,
      icon: <Icon name="plus" size={26} strokeWidth={2} />,
      center: true,
    },
    // 三审 C-3：messages 关闭则移动 TabBar 消息位替换为「我的」直连
    ...(mod("messages")
      ? [
          {
            href: "/messages",
            label: dict.nav.tabbarMessages,
            icon: <Icon name="messages" size={23} />,
          },
        ]
      : []),
    {
      href: "/my",
      label: dict.tabbar.my,
      icon: <Icon name="user" size={23} />,
    },
  ];
  return (
    <nav
      aria-label={dict.tabbar.ariaBottom}
      className="fixed inset-x-0 bottom-0 z-40 flex items-end border-t border-line bg-[var(--brand-paper)] pb-[env(safe-area-inset-bottom)] lg:hidden"
    >
      {tabs.map((t) =>
        t.center ? (
          /* 中间发布：极光渐变凸起大圆 —— 移动端核心行动召唤 */
          <TabLink
            key={t.href}
            href={t.href}
            label={t.label}
            className="-mt-6 flex flex-1 flex-col items-center justify-end gap-1 pb-1 text-[12px] font-bold text-sub"
          >
            <span
              aria-hidden
              className="flex h-14 w-14 items-center justify-center rounded-full border-4 border-[var(--brand-paper)] leading-none text-white shadow-[0_8px_20px_var(--accent-shadow)]"
              style={{ background: "var(--tide-sea, var(--sky))" }}
            >
              {t.icon}
            </span>
            {t.label}
          </TabLink>
        ) : (
          <TabLink
            key={t.href}
            href={t.href}
            label={t.label}
            className="tablink flex min-h-[44px] flex-1 flex-col items-center
              justify-center gap-0.5 py-1 text-[12px] text-sub active:text-link"
          >
            <span
              aria-hidden
              className="tablink-ic flex items-center justify-center
                leading-none"
            >
              {t.icon}
            </span>
            {t.label}
          </TabLink>
        ),
      )}
    </nav>
  );
}
