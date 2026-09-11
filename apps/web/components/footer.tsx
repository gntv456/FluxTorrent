import Link from "next/link";
import { getDict } from "@/i18n/server";
import { getSiteStats, type SiteStats } from "@/lib/data";
import { fmt } from "@/i18n/config";
import { formatBytes } from "@/lib/format";

/**
 * 包子站页脚复刻（参照 ref1-bot.png）：
 * 站点信息 / 快捷导航 / 帮助 三栏卡片 + 运行统计条 + 免责声明 + 版权条。
 * 统计接口需登录，未登录时该条自动隐藏（页面其余部分照常渲染）。
 */
export async function Footer() {
  const { dict } = await getDict();
  const stats = (await getSiteStats()) as SiteStats | null;
  const year = new Date().getFullYear();

  const links = [
    { href: "/", label: dict.nav.home },
    { href: "/torrents", label: dict.nav.library },
    { href: "/torrents?official=1", label: dict.nav.official },
    { href: "/forums", label: dict.nav.forums },
    { href: "/top", label: dict.nav.top },
    { href: "/requests", label: dict.nav.candidates },
  ];

  return (
    <footer className="mt-10 border-t border-line bg-[var(--baozi-bg)] pb-20 md:pb-6">
      <div className="mx-auto w-full max-w-[1536px] px-4 pt-6 md:px-6">
        {/* 三栏信息卡片 */}
        <div className="grid gap-4 md:grid-cols-3">
          <div className="rounded-[var(--r-md)] border border-[var(--baozi-line-soft)] bg-[var(--baozi-paper)] p-4 shadow-[var(--shadow-card)]">
            <h2 className="font-display text-sm font-bold text-ink">
              {dict.footer.aboutTitle}
            </h2>
            <p className="mt-2 flex items-center gap-2 text-sm text-sub">
              <span aria-hidden className="text-xl">
                🥟
              </span>
              {dict.footer.about}
            </p>
          </div>
          <div className="rounded-[var(--r-md)] border border-[var(--baozi-line-soft)] bg-[var(--baozi-paper)] p-4 shadow-[var(--shadow-card)]">
            <h2 className="font-display text-sm font-bold text-ink">
              {dict.footer.linksTitle}
            </h2>
            <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
              {links.map((l) => (
                <li key={l.href}>
                  <Link
                    href={l.href}
                    className="text-sm text-sky hover:text-[var(--baozi-orange)]"
                  >
                    {l.label}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
          <div className="rounded-[var(--r-md)] border border-[var(--baozi-line-soft)] bg-[var(--baozi-paper)] p-4 shadow-[var(--shadow-card)]">
            <h2 className="font-display text-sm font-bold text-ink">
              {dict.footer.helpTitle}
            </h2>
            <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
              <li>
                <Link
                  href="/rules"
                  className="text-sm text-sky hover:text-[var(--baozi-orange)]"
                >
                  {dict.footer.rules}
                </Link>
              </li>
              <li>
                <Link
                  href="/faq"
                  className="text-sm text-sky hover:text-[var(--baozi-orange)]"
                >
                  {dict.footer.faq}
                </Link>
              </li>
              <li>
                <Link
                  href="/ban-log"
                  className="text-sm text-sky hover:text-[var(--baozi-orange)]"
                >
                  {dict.footer.banlog}
                </Link>
              </li>
            </ul>
          </div>
        </div>

        {/* 运行统计条（登录后可见） */}
        {stats && (
          <div className="mt-4 flex flex-wrap items-center justify-center gap-x-6 gap-y-1 rounded-[var(--r-md)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-4 py-2 text-center text-xs font-bold text-[var(--baozi-orange-dark)]">
            <span>👥 {fmt(dict.footer.statsUsers, { n: stats.users })}</span>
            <span>
              🌱 {fmt(dict.footer.statsTorrents, { n: stats.torrents })}
            </span>
            <span>
              📦 {fmt(dict.footer.statsSeedSize, { n: formatBytes(stats.seed_size) })}
            </span>
            <span>💀 {fmt(dict.footer.deadTorrents, { n: stats.dead })}</span>
          </div>
        )}

        {/* 免责声明 + 版权条 */}
        <div className="mt-4 border-t border-dashed border-[var(--baozi-line-soft)] pt-3 text-center">
          <p className="text-[11px] leading-relaxed text-sub">
            {dict.footer.disclaimer}
          </p>
          <p className="mt-1 text-[11px] text-sub">
            {fmt(dict.footer.copyright, { year })}
          </p>
        </div>
      </div>
    </footer>
  );
}
