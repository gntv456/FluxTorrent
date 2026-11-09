"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "@/i18n/client";
import { ThemeToggle } from "@/components/theme-toggle";
import type { NavItem, NavGroup } from "@/lib/nav-menu";
import {
  buildNavTree,
  claimedPaths,
  isActive as navIsActive,
  leavesOf,
} from "@/lib/nav-tree";
import { useFocusTrap } from "@/lib/use-focus-trap";
import { ShjAtlasEduDrill } from "@/components/shj-atlas-edu";
import { ico } from "@/components/shj-atlas-icons";
import { ShjAtlasFoot } from "@/components/shj-atlas-foot";

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
  // 2026-11-09 重定：未存偏好时 ≥1180 默认展开（真实让位 236px），
  // 768–1179 由 CSS 恒为轨态；只有用户手动收轨（存 "1"）才在宽屏收起。
  const [rail, setRail] = useState(false);
  useEffect(() => {
    try {
      const saved = localStorage.getItem("flux-shj-rail");
      setRail(saved === "1");
    } catch {
      /* 隐私模式读不到就按默认展开 */
    }
  }, []);
  // 教育四级（0474）：钻取视图游标——null=关；stage:-1=学段页；
  // stage:N=年级页；grade:N=科目页。路由变化由本层关闭。
  const [eduDrill, setEduDrill] = useState<{
    stage: number;
    grade: number | null;
  } | null>(null);
  // 路由变化收起钻取视图（与抽屉/浮层同口径；首帧 null 无副作用）
  useEffect(() => setEduDrill(null), [pathname]);
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
  // ≥1180px 展开态与 <768px 抽屉态均不渲染（CSS 同口径隐藏）。
  // hover 打开不抢焦点；键盘/点击打开才启用焦点陷阱（沿用 useFocusTrap）。
  const [flyout, setFlyout] = useState<string | null>(null);
  const [flyoutTrap, setFlyoutTrap] = useState(false);
  const flyoutRef = useRef<HTMLDivElement>(null);
  // 焦点先于点击到达：若浮层是本焦点打开的，随后那次 click 不应再当「关」处理，
  // 否则鼠标点分组标题会「开了立刻关」（mousedown 触发 focus、click 又 toggle）。
  const openedByFocusRef = useRef(false);
  // 收起时 useFocusTrap 会把焦点还给分组标题，触发 onFocus 又把浮层打开
  // （Esc/点外收起后会「关不上」）。关闭动作置位一次，由下一次 onFocus 消费。
  const suppressFocusOpenRef = useRef(false);
  const closeFlyout = () => {
    suppressFocusOpenRef.current = true;
    setFlyout(null);
    setFlyoutTrap(false);
  };
  useFocusTrap(flyout !== null && flyoutTrap, flyoutRef);
  useEffect(() => {
    setFlyout(null);
    setFlyoutTrap(false);
  }, [pathname]);
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
      if (e.key === "Escape") {
        closeFlyout();
      }
    };
    const onDown = (e: PointerEvent) => {
      if (!(e.target as HTMLElement).closest(".shj-atlas")) {
        closeFlyout();
      }
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
  const sp = useMemo(() => new URLSearchParams(search), [search]);
  // 与顶栏/抽屉完全同源：同一棵树、同一「键值全等」口径
  const tree = useMemo(
    () => buildNavTree(primary, groups, dict.nav.more),
    [primary, groups, dict.nav.more],
  );
  const claimed = useMemo(
    () => claimedPaths(leavesOf(tree), sp),
    [tree, sp],
  );
  const isActive = (href: string) =>
    navIsActive(href, pathname, sp, claimed);
  // 教育资源项（0474）：按构建期打的 edu 标记取，不能猜首个带 sub 的项
  // ——资源库同样带 sub 且排在前面。
  const eduItem = primary.find((n) => n.edu) ?? null;

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
            active={isActive}
            onClose={() => setEduDrill(null)}
          />
        ) : (
          <ul className="shj-atlas__list shj-atlas__list--primary">
            {primary.map((n) => (
              <li key={n.href}>
                <ItemLink {...n} />
                {n.edu && (
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
              onMouseEnter={() => {
                if (isWide && !rail) return;
                openedByFocusRef.current = false;
                setFlyoutTrap(false);
                setFlyout(g.group);
              }}
              onMouseLeave={() => {
                if (!flyoutTrap) setFlyout(null);
              }}
            >
              <button
                type="button"
                className="shj-atlas__gtitle"
                aria-expanded={isWide && !rail ? !isClosed : isFly}
                onFocus={() => {
                  if (isWide && !rail) return;
                  if (suppressFocusOpenRef.current) {
                    suppressFocusOpenRef.current = false;
                    return;
                  }
                  if (flyout === g.group) return;
                  openedByFocusRef.current = true;
                  setFlyoutTrap(true);
                  setFlyout(g.group);
                }}
                onClick={() => {
                  if (isWide && !rail) {
                    setClosed((c) => ({ ...c, [g.group]: !c[g.group] }));
                  } else if (isFly) {
                    // 这次展开是同一交互里 focus 抢开的 → 消费掉，不关
                    if (openedByFocusRef.current) {
                      openedByFocusRef.current = false;
                      return;
                    }
                    closeFlyout();
                  } else {
                    openedByFocusRef.current = false;
                    setFlyoutTrap(true);
                    setFlyout(g.group);
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
                  ref={flyoutRef}
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
      <ShjAtlasFoot
        summary={summary}
        avaFailed={avaFailed}
        setAvaFailed={setAvaFailed}
        rail={rail}
        toggleRail={toggleRail}
        themeToggleLabel={dict.common.themeToggle}
        expandLabel={dict.nav.expandMenu}
        collapseLabel={dict.nav.collapseMenu}
      />
    </aside>
  );
}
