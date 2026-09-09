import Link from "next/link";

/** 桌面顶栏（设计稿：Logo + 搜索 + 导航 + 发布按钮 + 头像位，64px 吸顶） */
export function Header() {
  const nav = [
    { href: "/", label: "首页" },
    { href: "/torrents", label: "资源库" },
    { href: "/torrents?official=1", label: "官种" },
    { href: "/forums", label: "论坛" },
    { href: "/textbooks", label: "课本" },
    { href: "/medals", label: "勋章" },
    { href: "/top", label: "排行" },
    { href: "/magic-pool", label: "站免池" },
    { href: "/games", label: "娱乐屋" },
  ];
  return (
    <header className="sticky top-0 z-40 h-16 border-b border-line bg-ink/95 backdrop-blur">
      <div className="mx-auto flex h-full max-w-[1280px] items-center gap-6 px-4 md:px-6">
        <Link href="/" className="flex items-center gap-2">
          {/* 猫头鹰学士吉祥物占位（素材入库后替换为 next/image） */}
          <span aria-hidden className="text-2xl">🦉</span>
          <span className="font-display text-xl text-white">好学</span>
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
          <Link
            href="/upload"
            className="inline-flex min-h-[44px] items-center rounded-full bg-coral px-4 text-sm font-bold text-white shadow-[var(--shadow-hover)] transition-transform active:scale-[0.97]"
          >
            发布资源
          </Link>
          <Link
            href="/login"
            className="min-h-[44px] flex items-center text-sm text-white/80 hover:text-sky"
          >
            登录
          </Link>
        </div>
      </div>
    </header>
  );
}

/** 移动底部 5 Tab（设计稿 C 系列：图标 24px + 标签 11px，触控 ≥44px） */
export function MobileTabBar() {
  const tabs = [
    { href: "/", label: "首页", icon: "🏠" },
    { href: "/torrents", label: "搜索", icon: "🔍" },
    { href: "/upload", label: "发布", icon: "➕" },
    { href: "/my", label: "我的", icon: "👤" },
    { href: "/shop", label: "商店", icon: "🛍️" },
  ];
  return (
    <nav
      aria-label="底部导航"
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
