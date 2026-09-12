import Link from "next/link";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { LocaleSwitcher } from "@/components/locale-switcher";
import { ThemeToggle } from "@/components/theme-toggle";
import { UserBox } from "@/components/user-box";
import { MainMenu } from "@/components/main-menu";
import { CustomMenu } from "@/components/custom-menu";
import { getMenuItems } from "@/lib/data";

/** 导航项（Seedlight §3）：href + label；group 用于「更多 ▾」下拉分组。 */
type NavItem = { href: string; label: string };
type NavGroup = { group: string; items: NavItem[] };

/**
 * Seedlight 页头：单行导航条（logo 居左 + 一级/更多菜单居中 + 主题/语言切换居右）+ userbar。
 * 导航按「核心任务 / 发现 / 经济 / 成长 / 娱乐」四域收纳，桌面发布入口收进一级菜单，移动端走底部 5 Tab。
 */
export async function Header() {
  const { dict, locale } = await getDict();
  const profile = await getSiteProfile();
  const textbooksOn = profile.modules.textbooks === true;
  const brand = profile.brand || dict.common.brand;

  // 自定义菜单（location=topbar，nav.custom_enabled 开启时接口才返回非空）：
  // 一级项替换主菜单；带子项的一级项其子项收进「更多 ▾」作为独立分组（nexusphp-menu 生产口径）。
  // 接口返回空 = 开关关闭或未配置 → 显式回退默认导航（不做静默混淆）。
  const customItems = await getMenuItems("topbar");
  const customTops = customItems.filter((m) => m.parent_id === 0);
  const customActive = customTops.length > 0;
  const customGroups: NavGroup[] = customActive
    ? customTops
        .filter((m) => customItems.some((c) => c.parent_id === m.id))
        .map((m) => ({
          group: m.label,
          items: customItems
            .filter((c) => c.parent_id === m.id)
            .map((c) => ({ href: c.url, label: c.label })),
        }))
    : [];

  // 默认一级：核心任务域（发布入口在菜单内，不再放独立大按钮）
  const defaultPrimary: NavItem[] = [
    { href: "/", label: dict.nav.home },
    { href: "/torrents", label: dict.nav.library },
    { href: "/forums", label: dict.nav.forums },
    { href: "/top", label: dict.nav.top },
    { href: "/upload", label: dict.nav.upload },
  ];
  const primary: NavItem[] = customActive
    ? customTops.map((m) => ({ href: m.url, label: m.label }))
    : defaultPrimary;

  // 「更多 ▾」收纳域（自定义生效时替换为自定义子项分组）
  const groups: NavGroup[] = customActive
    ? customGroups
    : [
    {
      group: dict.nav.discover,
      items: [
        { href: "/torrents?official=1", label: dict.nav.official },
        { href: "/requests", label: dict.nav.candidates },
        { href: "/offers", label: dict.nav.offers },
        { href: "/subtitles", label: dict.nav.subtitles },
        { href: "/preserve", label: dict.nav.preserve },
        ...(textbooksOn ? [{ href: "/textbooks", label: dict.nav.textbooks }] : []),
      ],
    },
    {
      group: dict.nav.spark,
      items: [
        { href: "/shop", label: dict.tabbar.shop },
        { href: "/bank", label: dict.nav.bank },
        { href: "/magic-pool", label: dict.nav.magicPool },
        { href: "/tasks", label: dict.nav.tasks },
        { href: "/my-spark", label: dict.nav.spark },
      ],
    },
    {
      group: dict.nav.growth,
      items: [
        { href: "/medal-wall", label: dict.nav.medalWall },
        { href: "/avatar-frames", label: dict.nav.frames },
        { href: "/medals", label: dict.nav.medals },
        { href: "/jixiao", label: dict.nav.jixiao },
        { href: "/invites", label: dict.nav.invites },
      ],
    },
    {
      group: dict.nav.fun,
      items: [
        { href: "/games", label: dict.nav.games },
        { href: "/farm", label: dict.nav.farm },
        { href: "/gomoku", label: dict.nav.gomoku },
        { href: "/contests", label: dict.nav.contests },
        { href: "/friends", label: dict.nav.friends },
      ],
    },
    {
      group: dict.nav.more,
      items: [
        { href: "/messages", label: dict.nav.messages },
        { href: "/myhr", label: dict.nav.myhr },
        { href: "/faq", label: dict.nav.faq },
        { href: "/donate", label: dict.nav.donate },
      ],
    },
  ];

  return (
    <header className="border-b border-line bg-[var(--baozi-bg)]">
      {/* 1) 单行导航条：品牌（logo）居左 + 一级/更多菜单 + 主题/语言切换居右（菜单换行时允许整条长高） */}
      <div className="mx-auto flex min-h-[72px] w-full max-w-[1536px] items-center gap-3 px-4 py-2 md:gap-5 md:px-6">
        <Link href="/" className="flex shrink-0 items-center gap-2">
          <span aria-hidden className="text-3xl">
            🦉
          </span>
          <span className="font-display text-2xl text-ink">{brand}</span>
        </Link>
        <div className="min-w-0 flex-1">
          <MainMenu items={primary} groups={groups} moreLabel={`${dict.nav.more} ▾`} ariaLabel={dict.nav.ariaPrimary} />
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <ThemeToggle />
          <LocaleSwitcher current={locale} />
        </div>
      </div>
      {/* 自定义菜单（location=topbar）已在上方替换主菜单一级项，无独立行 */}
      {/* 3) 用户信息条（登录态客户端补齐） */}
      <div className="mx-auto w-full max-w-[1536px] px-6 pb-3 pt-2">
        <div className="userbar">
          <UserBox loginLabel={dict.common.login} />
        </div>
      </div>
    </header>
  );
}

/** 移动底部 5 Tab（Seedlight §3）：首页/发现/发布（极光凸起）/消息/我的 */
export async function MobileTabBar() {
  const { dict } = await getDict();
  const tabs = [
    { href: "/", label: dict.tabbar.home, icon: "🏠" },
    { href: "/torrents", label: dict.tabbar.search, icon: "🔍" },
    { href: "/upload", label: dict.tabbar.publish, icon: "＋", center: true },
    { href: "/messages", label: dict.nav.tabbarMessages, icon: "💬" },
    { href: "/my", label: dict.tabbar.my, icon: "👤" },
  ];
  return (
    <nav
      aria-label={dict.tabbar.ariaBottom}
      className="fixed inset-x-0 bottom-0 z-40 flex items-end border-t border-line bg-[var(--baozi-paper)] pb-[env(safe-area-inset-bottom)] md:hidden"
    >
      {tabs.map((t) =>
        t.center ? (
          /* 中间发布：极光渐变凸起大圆 —— 移动端核心行动召唤 */
          <Link
            key={t.href}
            href={t.href}
            aria-label={t.label}
            className="-mt-6 flex flex-1 flex-col items-center justify-end gap-1 pb-1 text-[11px] font-bold text-sub"
          >
            <span
              aria-hidden
              className="flex h-14 w-14 items-center justify-center rounded-full border-4 border-[var(--baozi-paper)] text-2xl leading-none text-white shadow-[0_8px_20px_var(--accent-shadow)]"
              style={{ background: "var(--grad-aurora)" }}
            >
              {t.icon}
            </span>
            {t.label}
          </Link>
        ) : (
          <Link
            key={t.href}
            href={t.href}
            className="flex min-h-[44px] flex-1 flex-col items-center justify-center gap-0.5 py-1 text-[11px] text-sub active:text-sky"
          >
            <span aria-hidden className="text-[24px] leading-none">
              {t.icon}
            </span>
            {t.label}
          </Link>
        ),
      )}
    </nav>
  );
}
