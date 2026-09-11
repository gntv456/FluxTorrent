import Link from "next/link";
import { getDict } from "@/i18n/server";
import { LocaleSwitcher } from "@/components/locale-switcher";
import { UserBox } from "@/components/user-box";
import { MainMenu } from "@/components/main-menu";

/**
 * 包子站三段式页头复刻：
 * 1) logo 行（站标 + 语言切换 + 发布按钮）
 * 2) #mainmenu 卡片导航条（当前页橙色渐变高亮）
 * 3) userbar 用户信息条（欢迎回来 + 魔力胶囊 + 票券统计，UserBox 客户端补齐）
 */
export async function Header() {
  const { dict, locale } = await getDict();
  const nav = [
    { href: "/", label: dict.nav.home },
    { href: "/torrents", label: dict.nav.library },
    { href: "/torrents?official=1", label: dict.nav.official },
    { href: "/forums", label: dict.nav.forums },
    { href: "/requests", label: dict.nav.candidates },
    { href: "/offers", label: dict.nav.offers },
    { href: "/preserve", label: dict.nav.preserve },
    { href: "/upload", label: dict.nav.upload },
    { href: "/games", label: dict.nav.games },
    { href: "/top", label: dict.nav.top },
    { href: "/messages", label: dict.nav.messages },
    { href: "/medals", label: dict.nav.medals },
    { href: "/tasks", label: dict.nav.tasks },
    { href: "/bank", label: dict.nav.bank },
    { href: "/invites", label: dict.nav.invites },
    { href: "/subtitles", label: dict.nav.subtitles },
    { href: "/friends", label: dict.nav.friends },
    { href: "/textbooks", label: dict.nav.textbooks },
    { href: "/magic-pool", label: dict.nav.magicPool },
    { href: "/myhr", label: dict.nav.myhr },
    { href: "/contests", label: dict.nav.contests },
    { href: "/medal-wall", label: dict.nav.medalWall },
    { href: "/avatar-frames", label: dict.nav.frames },
    { href: "/gomoku", label: dict.nav.gomoku },
    { href: "/faq", label: dict.nav.faq },
  ];
  return (
    <header className="border-b border-line bg-[var(--baozi-bg)]">
      {/* 1) logo 行 */}
      <div className="mx-auto flex h-[86px] w-full max-w-[1536px] items-center justify-between px-6">
        <Link href="/" className="flex items-center gap-3">
          <span aria-hidden className="text-4xl">
            🥟
          </span>
          <span className="font-display text-3xl text-ink">{dict.common.brand}</span>
        </Link>
        <div className="flex items-center gap-3">
          <LocaleSwitcher current={locale} />
          <Link
            href="/upload"
            className="flex min-h-[44px] items-center rounded-[10px] border border-[var(--baozi-orange-dark)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-4 text-sm font-bold text-white shadow-[var(--shadow-hover)] transition-transform active:scale-[0.97]"
          >
            {dict.common.publish}
          </Link>
        </div>
      </div>
      {/* 2) 卡片导航条（客户端组件跟踪路径，当前页橙色渐变高亮） */}
      <div className="mx-auto w-full max-w-[1536px] px-6">
        <MainMenu items={nav} ariaLabel={dict.nav.ariaPrimary} />
      </div>
      {/* 3) 用户信息条（登录态客户端补齐） */}
      <div className="mx-auto w-full max-w-[1536px] px-6 pb-3 pt-2">
        <div className="userbar">
          <UserBox loginLabel={dict.common.login} />
        </div>
      </div>
    </header>
  );
}

/** 移动底部 5 Tab（图标 24px + 标签 11px，触控 ≥44px） */
export async function MobileTabBar() {
  const { dict } = await getDict();
  const tabs = [
    { href: "/", label: dict.tabbar.home, icon: "🏠" },
    { href: "/torrents", label: dict.tabbar.search, icon: "🔍" },
    { href: "/upload", label: dict.tabbar.publish, icon: "➕" },
    { href: "/my", label: dict.tabbar.my, icon: "👤" },
    { href: "/shop", label: dict.tabbar.shop, icon: "🛍️" },
  ];
  return (
    <nav
      aria-label={dict.tabbar.ariaBottom}
      className="fixed inset-x-0 bottom-0 z-40 flex border-t border-line bg-[var(--baozi-paper)] pb-[env(safe-area-inset-bottom)] md:hidden"
    >
      {tabs.map((t) => (
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
      ))}
    </nav>
  );
}
