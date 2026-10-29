import Link from "next/link";
import { PANEL_BRAND } from "@/lib/ui-classes";
import { getDict } from "@/i18n/server";
import { getSiteStats, getMenuItems, type SiteStats } from "@/lib/data";
import { fmt } from "@/i18n/config";
import { formatBytes } from "@/lib/format";
import { MenuItemLink } from "@/components/custom-menu";
import { getSiteProfile } from "@/lib/site-profile";
import { api } from "@/lib/api-client";

/** 帮助区/快捷导航共用的链接样式（长类名抽常量守行宽门禁） */
const FOOTER_LINK =
  "inline-block py-1.5 text-sm text-link hover:text-[var(--brand-orange)]";

/**
 * 页脚：
 * 站点信息 / 快捷导航 / 帮助 三栏卡片 + 运行统计条 + 免责声明 + 版权条。
 * 统计接口需登录，未登录时该条自动隐藏（页面其余部分照常渲染）。
 * IA-5（DS-PLAN-01 W-11）：版权条带程序版本（/api/v1/about 的
 * product+version）——六站横评三家页脚直显版本，是行业惯例；
 * 版本拉取失败不影响页脚其余部分（about 接口失败时省略版本号）。
 */
export async function Footer() {
  const { dict } = await getDict();
  const stats = (await getSiteStats()) as SiteStats | null;
  const customLinks = await getMenuItems("footer");
  const year = new Date().getFullYear();
  // 版权条动态化：站名（site_name）+ 建站日期（datefounded）——通用建站口径，
  // 不再内嵌任何具体站名；日期缺省退当前年份
  const profile = await getSiteProfile();
  const siteName = profile.brand || "FluxTorrent";
  const foundedDate = profile.founded || String(year);
  // 帮助区链接（0423 补 /clients）：四条同构，map 渲染守行宽门禁
  const helpLinks = [
    { href: "/rules", label: dict.footer.rules },
    { href: "/faq", label: dict.footer.faq },
    { href: "/ban-log", label: dict.footer.banlog },
    { href: "/clients", label: dict.footer.clients },
  ];
  // IA-5：版本号与 about 页同源（/api/v1/about），失败静默省略
  const about = await api
    .get<{ product: string; version: string }>("/api/v1/about")
    .catch(() => null);

  // 快捷导航按模块开关过滤（二审 G2-8 修复）：forums/requests 关闭的站
  // 页脚不再保留死入口；缺键视为开（与 requireModule 口径一致）
  const mod = (k: string) => profile.modules[k] !== false;
  const links = [
    { href: "/", label: dict.nav.home },
    { href: "/torrents", label: dict.nav.library },
    { href: "/torrents?official=1", label: dict.nav.official },
    ...(mod("forums") ? [{ href: "/forums", label: dict.nav.forums }] : []),
    { href: "/top", label: dict.nav.top },
    ...(mod("requests")
      ? [{ href: "/requests", label: dict.nav.requests }]
      : []),
  ];

  return (
    <footer className="mt-10 border-t border-line bg-[var(--brand-bg)] pb-20 lg:pb-6">
      <div className="mx-auto w-full max-w-[1536px] px-4 pt-6 md:px-6">
        {/* 三栏信息卡片 */}
        <div className="grid gap-4 md:grid-cols-3">
          <div className={PANEL_BRAND}>
            <h2 className="font-display text-sm font-bold text-ink">
              {dict.footer.aboutTitle}
            </h2>
            <p className="mt-2 flex items-start gap-2 text-sm text-sub">
              <span aria-hidden className="text-xl">
                🌱
              </span>
              {/* 站点简介（0088）：后台 site_settings.site_desc 可配，留空回落字典默认 */}
              {profile.site_desc || dict.footer.about}
            </p>
          </div>
          <div className={PANEL_BRAND}>
            <h2 className="font-display text-sm font-bold text-ink">
              {dict.footer.linksTitle}
            </h2>
            <ul className="mt-1 flex flex-wrap gap-x-4">
              {links.map((l) => (
                <li key={l.href}>
                  <Link
                    href={l.href}
                    className={FOOTER_LINK}
                  >
                    {l.label}
                  </Link>
                </li>
              ))}
              {/* 自定义菜单（location=footer）：追加在快捷导航末尾 */}
              {customLinks.map((m) => (
                <li key={`m${m.id}`}>
                  <MenuItemLink
                    item={m}
                    className={FOOTER_LINK}
                  />
                </li>
              ))}
            </ul>
          </div>
          <div className={PANEL_BRAND}>
            <h2 className="font-display text-sm font-bold text-ink">
              {dict.footer.helpTitle}
            </h2>
            <ul className="mt-1 flex flex-wrap gap-x-4">
              {helpLinks.map((l) => (
                <li key={l.href}>
                  <Link
                    href={l.href}
                    className={FOOTER_LINK}
                  >
                    {l.label}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
        </div>

        {/* 运行统计条（登录后可见） */}
        {stats && (
          <div className="mt-4 flex flex-wrap items-center justify-center gap-x-6 gap-y-1 rounded-[var(--r-md)] border border-[var(--brand-line)] bg-[var(--brand-paper)] px-4 py-2 text-center text-xs font-bold text-[var(--brand-orange-dark)]">
            <span>👥 {fmt(dict.footer.statsUsers, { n: stats.users })}</span>
            <span>
              🌱 {fmt(dict.footer.statsTorrents, { n: stats.torrents })}
            </span>
            <span>
              📦{" "}
              {fmt(dict.footer.statsSeedSize, {
                n: formatBytes(stats.seed_size),
              })}
            </span>
            {/* 全断种降级（0283 P2）：种子全断且做种总量为 0 时，「无种 N」
                与「N 种子」并排观感差（冷启动站点第一天全死）。改发种引导。 */}
            {stats.torrents > 0 &&
            stats.dead === stats.torrents &&
            stats.seed_size === 0 ? (
              <Link href="/upload" className="underline decoration-dotted">
                {dict.footer.allDeadHint}
              </Link>
            ) : (
              <span>💀 {fmt(dict.footer.deadTorrents, { n: stats.dead })}</span>
            )}
          </div>
        )}

        {/* 免责声明 + 版权条 */}
        <div className="mt-4 border-t border-dashed border-[var(--brand-line-soft)] pt-3 text-center">
          <p className="text-[12px] leading-relaxed text-sub">
            {dict.footer.disclaimer}
          </p>
          <p className="mt-1 text-[12px] text-sub">
            {fmt(dict.footer.copyright, {
              site: siteName,
              date: foundedDate,
            })}{" "}
            <Link
              href="/about"
              className="text-link hover:text-[var(--brand-orange)]"
            >
              {about
                ? `Powered by ${about.product} ${about.version}`
                : "Powered by FluxTorrent"}
            </Link>
          </p>
        </div>
      </div>
    </footer>
  );
}
