import Link from "next/link";
import { Suspense } from "react";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { LocaleSwitcher } from "@/components/locale-switcher";
import { ThemeToggle } from "@/components/theme-toggle";
import { UserMenu } from "@/components/user-menu";
import { MainMenu } from "@/components/main-menu";
import { CustomMenu } from "@/components/custom-menu";
import { Icon } from "@/components/icons";
import { getMenuItems } from "@/lib/data";

/** 导航项（Seedlight §3）：href + label；group 用于「更多 ▾」下拉分组。 */
type NavItem = { href: string; label: string };
type NavGroup = { group: string; items: NavItem[] };

/**
 * Seedlight 页头：单行导航条（logo 居左 + 一级/更多菜单居中 + 主题/语言/头像弹窗居右）。
 * 用户栏（userbar）已收进头像弹窗（0147，好学 CuteTop 口径）——导航条整条吸顶。
 */
export async function Header() {
  const { dict, locale, currency } = await getDict();
  const profile = await getSiteProfile();
  /** 模块开关（U1 §6.2）：缺键视为开（T3 缺省=现状），与 API 侧 default_on 口径一致 */
  const mod = (k: string) => profile.modules[k] !== false;
  const textbooksOn = mod("textbooks");
  /** 社交层（0102）：由 site_settings.module_social 决定，关闭时导航里不出现入口 */
  const socialOn = mod("social");
  const brand = profile.brand || dict.common.brand;
  /** 字典文案里的 {magic} 货币占位符（如 nav.spark「{magic}经济」）在此替换为站点货币名 */
  const t = (tpl: string) => tpl.replace("{magic}", currency);

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
  // 三审 C-3：头部主导航与 footer 同口径——forums 关闭则一级入口消失
  const defaultPrimary: NavItem[] = [
    { href: "/", label: dict.nav.home },
    { href: "/torrents", label: dict.nav.library },
    ...(mod("forums") ? [{ href: "/forums", label: dict.nav.forums }] : []),
    { href: "/top", label: dict.nav.top },
    { href: "/upload", label: dict.nav.upload },
  ];
  const primary: NavItem[] = customActive
    ? customTops.map((m) => ({ href: m.url, label: m.label }))
    : defaultPrimary;

  // 「更多 ▾」收纳域（自定义生效时替换为自定义子项分组；条目按 module 开关过滤 U1 §6.2）
  const groups: NavGroup[] = customActive
    ? customGroups
    : [
        {
          group: dict.nav.discover,
          items: [
            { href: "/torrents?official=1", label: dict.nav.official },
            ...(mod("requests")
              ? [{ href: "/requests", label: dict.nav.requests }]
              : []),
            ...(mod("offers")
              ? [{ href: "/offers", label: dict.nav.offers }]
              : []),
            ...(mod("subtitles")
              ? [{ href: "/subtitles", label: dict.nav.subtitles }]
              : []),
            ...(mod("preserve")
              ? [{ href: "/preserve", label: dict.nav.preserve }]
              : []),
            ...(socialOn
              ? [
                  { href: "/endangered", label: dict.nav.endangered },
                  { href: "/teams", label: dict.nav.teams },
                ]
              : []),
            ...(textbooksOn
              ? [{ href: "/textbooks", label: dict.nav.textbooks }]
              : []),
          ],
        },
        {
          group: t(dict.nav.spark),
          items: [
            ...(mod("shop")
              ? [{ href: "/shop", label: dict.tabbar.shop }]
              : []),
            ...(mod("bank") ? [{ href: "/bank", label: dict.nav.bank }] : []),
            ...(mod("magic_pool")
              ? [{ href: "/magic-pool", label: dict.nav.magicPool }]
              : []),
            ...(mod("tasks")
              ? [{ href: "/tasks", label: dict.nav.tasks }]
              : []),
            { href: "/my-spark", label: t(dict.nav.spark) },
          ],
        },
        {
          group: dict.nav.growth,
          items: [
            ...(mod("exams")
              ? [{ href: "/me/exams", label: dict.nav.exams }]
              : []),
            { href: "/me/achievements", label: dict.nav.achievements },
            ...(mod("resurrections")
              ? [{ href: "/resurrections", label: dict.nav.resurrections }]
              : []),
            ...(mod("medals")
              ? [{ href: "/medal-wall", label: dict.nav.medalWall }]
              : []),
            ...(mod("dressup")
              ? [{ href: "/avatar-frames", label: dict.nav.frames }]
              : []),
            ...(mod("medals")
              ? [{ href: "/medals", label: dict.nav.medals }]
              : []),
            ...(mod("jixiao")
              ? [{ href: "/jixiao", label: dict.nav.jixiao }]
              : []),
            ...(mod("invites")
              ? [{ href: "/invites", label: dict.nav.invites }]
              : []),
          ],
        },
        {
          group: dict.nav.fun,
          items: [
            ...(mod("games")
              ? [{ href: "/games", label: dict.nav.games }]
              : []),
            ...(mod("farm") ? [{ href: "/farm", label: dict.nav.farm }] : []),
            ...(mod("gomoku")
              ? [{ href: "/gomoku", label: dict.nav.gomoku }]
              : []),
            ...(mod("contests")
              ? [{ href: "/contests", label: dict.nav.contests }]
              : []),
            ...(mod("friends")
              ? [{ href: "/friends", label: dict.nav.friends }]
              : []),
          ],
        },
        {
          group: dict.nav.more,
          items: [
            ...(mod("messages")
              ? [{ href: "/messages", label: dict.nav.messages }]
              : []),
            // H&R 入口归 exams 键（/me/hr 网关同口径，二审 G2-3）
            ...(mod("exams") ? [{ href: "/myhr", label: dict.nav.myhr }] : []),
            { href: "/faq", label: dict.nav.faq },
            ...(mod("magic_pool")
              ? [{ href: "/donate", label: dict.nav.donate }]
              : []),
          ],
        },
      ];

  return (
    <>
      <header className="tide-header border-b border-line">
        {/* 单行导航条：品牌（logo）居左 + 一级/更多菜单 + 主题/语言/头像弹窗居右 */}
        <div className="mx-auto flex min-h-[72px] w-full max-w-[1536px] items-center gap-3 px-4 py-2 md:gap-5 md:px-6">
          <Link href="/" className="flex shrink-0 items-center gap-2">
            {/* 品牌标记：Tide 图标（原为猫头鹰 emoji）—— 颜色跟随 --sky，浅色/夜间自适应 */}
            <Icon
              name="seed"
              size={30}
              className="shrink-0 text-[var(--sky)]"
            />
            <span className="font-display text-2xl text-ink">{brand}</span>
          </Link>
          <div className="min-w-0 flex-1">
            {/* Suspense：MainMenu 用 useSearchParams 区分带参条目（官种），
                预渲染路由要求它包在边界里 */}
            <Suspense fallback={null}>
              <MainMenu
                items={primary}
                groups={groups}
                moreLabel={`${dict.nav.more} ▾`}
                ariaLabel={dict.nav.ariaPrimary}
              />
            </Suspense>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <ThemeToggle />
            {/* 语言切换器（0209 P2-17）：site_settings.locale_switcher_enabled=no 隐藏（单语站） */}
            {profile.locale_switcher_enabled !== "no" && (
              <LocaleSwitcher current={locale} />
            )}
            {/* 头像弹窗（0147）：点击头像展开用户下拉（摘要/数据/快捷/信箱） */}
            <UserMenu loginLabel={dict.common.login} />
          </div>
        </div>
      </header>
      {/* 自定义菜单（location=topbar）已在上方替换主菜单一级项，无独立行 */}
    </>
  );
}

/** 移动底部 5 Tab（Seedlight §3）：首页/发现/发布（极光凸起）/消息/我的 */
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
              className="flex h-14 w-14 items-center justify-center rounded-full border-4 border-[var(--baozi-paper)] leading-none text-white shadow-[0_8px_20px_var(--accent-shadow)]"
              style={{ background: "var(--tide-sea, var(--sky))" }}
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
            <span
              aria-hidden
              className="flex items-center justify-center leading-none"
            >
              {t.icon}
            </span>
            {t.label}
          </Link>
        ),
      )}
    </nav>
  );
}
