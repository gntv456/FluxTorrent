import Link from "next/link";
import { getDict } from "@/i18n/server";
import { LocaleSwitcher } from "@/components/locale-switcher";
import { UserBox } from "@/components/user-box";

/** 桌面顶栏（设计稿：Logo + 搜索 + 导航 + 发布按钮 + 头像位，64px 吸顶） */
export async function Header() {
  const { dict, locale } = await getDict();
  const nav = [
    { href: "/", label: dict.nav.home },
    { href: "/torrents", label: dict.nav.library },
    { href: "/torrents?official=1", label: dict.nav.official },
    { href: "/forums", label: dict.nav.forums },
    { href: "/messages", label: dict.nav.messages },
    { href: "/textbooks", label: dict.nav.textbooks },
    { href: "/medals", label: dict.nav.medals },
    { href: "/top", label: dict.nav.top },
    { href: "/magic-pool", label: dict.nav.magicPool },
    { href: "/games", label: dict.nav.games },
    { href: "/farm", label: dict.nav.farm },
    { href: "/dressup", label: dict.nav.dressup },
  ];
  return (
    <header className="sticky top-0 z-40 h-16 border-b border-line bg-ink/95 backdrop-blur">
      <div className="mx-auto flex h-full max-w-[1280px] items-center gap-6 px-4 md:px-6">
        <Link href="/" className="flex items-center gap-2">
          {/* 猫头鹰学士吉祥物占位（素材入库后替换为 next/image） */}
          <span aria-hidden className="text-2xl">
            🦉
          </span>
          <span className="font-display text-xl text-white">
            {dict.common.brand}
          </span>
        </Link>
        <nav className="hidden items-center gap-4 md:flex" aria-label="主导航">
          {nav.map((n) => (
            <Link
              key={n.href}
              href={n.href}
              className="min-h-[44px] flex items-center text-sm text-white/80 transition-colors hover:text-sky"
            >
              {n.label}
            </Link>
          ))}
        </nav>
        <div className="ml-auto flex items-center gap-3">
          <LocaleSwitcher current={locale} />
          <Link
            href="/upload"
            className="hidden min-h-[44px] items-center rounded-full bg-coral px-4 text-sm font-bold text-white shadow-[var(--shadow-hover)] transition-transform active:scale-[0.97] sm:inline-flex"
          >
            {dict.common.publish}
          </Link>
          <UserBox loginLabel={dict.common.login} />
        </div>
      </div>
    </header>
  );
}

/** 移动底部 5 Tab（设计稿 C 系列：图标 24px + 标签 11px，触控 ≥44px） */
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
      className="fixed inset-x-0 bottom-0 z-40 flex border-t border-line bg-white pb-[env(safe-area-inset-bottom)] md:hidden"
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
