"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { ThemeToggle } from "@/components/theme-toggle";
import type { NavItem, NavGroup } from "@/lib/nav-menu";
import { ShjAtlasEduDrill } from "@/components/shj-atlas-edu";

/** 山海经「山海图鉴」竖排侧栏（乙 · 结构层）。
 *
 *  数据来源与顶栏/抽屉/底 Tab 完全同源（lib/nav-menu.ts 的 buildNav 产物），
 *  入口一个不缺、命名零漂移。组件本身永远渲染（CSS 控制显隐）：
 *  仅 [data-theme="shanhaijing"] 下出现，其余三主题 display:none、零影响。
 *  三态全部 CSS 驱动（同一份 DOM）：
 *    ≥1180px 展开（236px，分组可折叠）；768–1179px 图标轨（64px，title 提示）；
 *    <768px 隐藏（导航由既有抽屉 + 底部 TabBar 承担，与生产 M2 一致）。
 *
 *  选中态与 MainMenu 同口径（裸路径给带参条目让位），但不消费
 *  useSearchParams——它会要求调用方包 Suspense；改用挂载后读
 *  window.location.search，结果一致且对路由组零侵入。 */

/** href → 图标（emoji，对齐设计样图 V1.2 图鉴图标位）。46 项逐项字面量
 *  会挤爆行数门禁，存成空白分隔的「路径 表情」对，启动期解析成 Map。 */
const ICON_TOKS = `
/ 🏠 /torrents 📚 /forums 💬 /upload 📤 /top 🏆 /requests 🙏 /offers 🎁
/networks 📡 /artists 🎤 /seasonal 🌸 /subtitles 📑 /subtitle-groups 👥
/preserve 🛡️ /endangered 🆘 /teams 🤝 /textbooks 📖 /shop 🛒 /bank 🏦
/magic-pool 🔮 /tasks ✅ /my-spark ✨ /me/sparks 📒 /classes 🎓 /me/exams 📝
/me/achievements 🏅 /resurrections 💫 /medal-wall 🎖️ /avatar-frames 🖼️
/medals 🏵️ /jixiao 📊 /invites ✉️ /games 🎮 /gacha 🥚 /farm 🌾 /gomoku ⚫
/contests 🏁 /friends 🧑‍🤝‍🧑 /messages 📨 /myhr ⚖️ /me/subscriptions 📺
/faq ❓ /clients 🧭 /wiki 📔 /donate 💝
`
  .split(/\s+/)
  .filter(Boolean);
const ICONS = new Map<string, string>();
for (let i = 0; i < ICON_TOKS.length; i += 2) {
  ICONS.set(ICON_TOKS[i], ICON_TOKS[i + 1]);
}

function ico(href: string): string {
  const path = href.split("?")[0];
  if (href.includes("official=1")) return "🏅";
  return ICONS.get(path) ?? "🔹";
}

export function ShjAtlas({
  primary,
  groups,
  summary,
  brand,
}: {
  primary: NavItem[];
  groups: NavGroup[];
  summary: {
    username: string;
    className: string;
    uploaded: string;
    avatarUrl?: string | null;
  } | null;
  brand: string;
}) {
  const pathname = usePathname();
  const { dict } = useI18n();
  // 境界签真头像（0480）：/me/overview 的 avatar_url 有图就渲染，加载失败
  // 回落首字母印章（与 user-menu 同一套 onError 语义）；换号（URL 变化）重置
  const [avaFailed, setAvaFailed] = useState(false);
  useEffect(() => setAvaFailed(false), [summary?.avatarUrl]);
  // 折叠组：默认全展开；key = 组名（同源数据，组名稳定）
  const [closed, setClosed] = useState<Record<string, boolean>>({});
  // 整栏折叠（0470）：宽屏用户手动在「236px 展开 ↔ 64px 图标轨」间切换，
  // localStorage 持久化（与主题偏好同款模式）；轨态复用 768–1179 断点的
  // 图标居中/浮层样式（.is-rail 类承载，CSS 同源）。
  // 2026-11-09 统一顶栏导航：默认收轨（null=用户没存过偏好时收起），
  // 顶栏 hover 级联承载全部导航，侧栏降为快捷浮层不再占位。
  const [rail, setRail] = useState(false);
  useEffect(() => {
    try {
      const saved = localStorage.getItem("flux-shj-rail");
      setRail(saved === null ? true : saved === "1");
    } catch {
      /* 隐私模式读不到就默认收轨 */
    }
  }, []);
  // 教育四级（0474）：钻取视图游标——null=关；stage:-1=学段页；
  // stage:N=年级页；grade:N=科目页。路由变化由钻取组件自行关闭。
  const [eduDrill, setEduDrill] = useState<{
    stage: number;
    grade: number | null;
  } | null>(null);
  const toggleRail = () =>
    setRail((r) => {
      try {
        localStorage.setItem("flux-shj-rail", r ? "0" : "1");
      } catch {
        /* 写不进不拦切换 */
      }
      return !r;
    });
  // 图标轨（768–1179px）分组浮层：点按/悬浮展开带文字的第二列；
  // ≥1180px 展开态与 <768px 抽屉态均不渲染（CSS 同口径隐藏）
  const [flyout, setFlyout] = useState<string | null>(null);
  useEffect(() => setFlyout(null), [pathname]);
  // 视口模式：≥1180px 展开态（折叠语义）/ 轨态（浮层语义）——
  // aria-expanded 与点击行为都以它为准（SSR 首帧按桌面展开，挂载校正）
  const [isWide, setIsWide] = useState(true);
  useEffect(() => {
    const mq = window.matchMedia("(min-width: 1180px)");
    const sync = () => setIsWide(mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, []);
  // Esc 收起 + 点外部收起（展开态保持 236px 不变，浮层只在轨态可见）
  useEffect(() => {
    if (!flyout) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setFlyout(null);
    };
    const onDown = (e: PointerEvent) => {
      if (!(e.target as HTMLElement).closest(".shj-atlas")) setFlyout(null);
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [flyout]);
  // 选中态的 query 部分：挂载后从真实地址读（避开 useSearchParams 的 Suspense 要求）
  const [search, setSearch] = useState("");
  useEffect(() => {
    setSearch(window.location.search);
  }, [pathname]);
  const sp = new URLSearchParams(search);
  // 教育资源项（0474）：唯一带 sub 的一级项
  const eduItem = primary.find((n) => n.sub) ?? null;

  const pathActive = (p: string) =>
    p === "/"
      ? pathname === "/"
      : pathname === p || pathname.startsWith(`${p}/`);
  const queryActive = (href: string) => {
    const q = href.split("?")[1];
    if (!q) return false;
    for (const [k, v] of new URLSearchParams(q)) {
      if (sp.get(k) !== v) return false;
    }
    return true;
  };
  const all = [...primary, ...groups.flatMap((g) => g.items)];
  const claimed = new Set(
    all.filter((i) => queryActive(i.href)).map((i) => i.href.split("?")[0]),
  );
  const isActive = (href: string) => {
    const [path, query] = href.split("?");
    if (!pathActive(path)) return false;
    if (query) return queryActive(href);
    return !claimed.has(path);
  };

  const ItemLink = ({ href, label }: NavItem) => (
    <Link
      href={href}
      title={label}
      className="shj-atlas__item"
      aria-current={isActive(href) ? "page" : undefined}
    >
      <span className="shj-atlas__ico" aria-hidden>
        {ico(href)}
      </span>
      <span className="shj-atlas__label">{label}</span>
    </Link>
  );

  return (
    <aside
      className={`shj-atlas${rail ? " is-rail" : ""}`}
      aria-label={dict.nav.ariaPrimary}
    >
      <Link href="/" className="shj-atlas__brand" title={brand}>
        <span className="shj-atlas__seal" aria-hidden>
          {brand.slice(0, 1)}
        </span>
        <span className="shj-atlas__brandtext">
          <b>{brand}</b>
          <i>{dict.common.themeShanhaijing}</i>
        </span>
      </Link>
      <nav className="shj-atlas__scroll">
        {eduDrill && eduItem ? (
          <ShjAtlasEduDrill
            sub={eduItem.sub!}
            onClose={() => setEduDrill(null)}
          />
        ) : (
          <ul className="shj-atlas__list shj-atlas__list--primary">
            {primary.map((n) => (
              <li key={n.href}>
                <ItemLink {...n} />
                {n.sub && (
                  <button
                    type="button"
                    className="shj-atlas__eduedit"
                    onClick={() => setEduDrill({ stage: -1, grade: null })}
                    title={n.label}
                  >
                    <span>{dict.nav.eduBrowse}</span>
                    <span aria-hidden>›</span>
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
        {groups.map((g) => {
          const isClosed = !!closed[g.group];
          const isFly = flyout === g.group;
          return (
            <section
              key={g.group}
              className={`shj-atlas__group${isClosed ? " is-closed" : ""}`}
              onMouseEnter={() => setFlyout(g.group)}
              onMouseLeave={() => setFlyout(null)}
            >
              <button
                type="button"
                className="shj-atlas__gtitle"
                aria-expanded={isWide && !rail ? !isClosed : isFly}
                onClick={() => {
                  if (isWide && !rail) {
                    setClosed((c) => ({ ...c, [g.group]: !c[g.group] }));
                  } else {
                    setFlyout(isFly ? null : g.group);
                  }
                }}
              >
                <span className="shj-atlas__gname">{g.group}</span>
                <span className="shj-atlas__gline" aria-hidden />
                <span className="shj-atlas__gcaret" aria-hidden>
                  ▾
                </span>
              </button>
              <ul className="shj-atlas__list" hidden={isClosed}>
                {g.items.map((n) => (
                  <li key={n.href + n.label}>
                    <ItemLink {...n} />
                  </li>
                ))}
              </ul>
              {/* 轨态浮层：同组条目带文字的第二列（仅 768–1179px 显示） */}
              {isFly && (
                <div
                  className="shj-atlas__flyout"
                  role="group"
                  aria-label={g.group}
                >
                  <p className="shj-atlas__ftitle">{g.group}</p>
                  <ul className="shj-atlas__list">
                    {g.items.map((n) => (
                      <li key={n.href + n.label}>
                        <ItemLink {...n} />
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </section>
          );
        })}
      </nav>
      <div className="shj-atlas__foot">
        {summary && (
          <Link
            href="/my"
            className="shj-atlas__realm"
            title={summary.username}
          >
            <span className="shj-atlas__rseal" aria-hidden>
              {summary.avatarUrl && !avaFailed ? (
                <img
                  src={summary.avatarUrl}
                  alt=""
                  onError={() => setAvaFailed(true)}
                />
              ) : (
                summary.username.slice(0, 1).toUpperCase()
              )}
            </span>
            <span className="shj-atlas__rtext">
              <b>{summary.username}</b>
              <i>
                {summary.className} · ↑ {summary.uploaded}
              </i>
            </span>
          </Link>
        )}
        <ThemeToggle label={dict.common.themeToggle} />
        {/* 整栏折叠钮（宽屏专属；窄断点由 CSS 隐藏——轨态本就是那段的缺省） */}
        <button
          type="button"
          className="shj-atlas__fold"
          onClick={toggleRail}
          aria-expanded={!rail}
          title={rail ? dict.nav.expandMenu : dict.nav.collapseMenu}
        >
          <span aria-hidden>{rail ? "»" : "«"}</span>
          <span className="shj-atlas__foldtext">
            {rail ? dict.nav.expandMenu : dict.nav.collapseMenu}
          </span>
        </button>
      </div>
    </aside>
  );
}
