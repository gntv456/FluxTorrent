"use client";

import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { useMediaQuery } from "@/lib/hooks/use-media";

/** 导航（Seedlight §3）：一级平铺 + 「更多 ▾」分组下拉（点击展开，点外部/ESC 收起）。
 *
 *  选中态口径（2026-11-03 收敛）：带参条目（官种 /torrents?official=1、
 *  资源库三级叶子 /torrents?category_id=7&official=1）按「键值全等」点亮
 *  ——URL 与菜单项的筛选参数完全一致才亮，子集不算（子集口径下选
 *  「资源-电影」会同时点亮全部-电影/资源-全部/资源-更多分类…四条）。
 *  裸路径条目（资源库 /torrents）在任何带参条目命中时让位。
 *
 *  平板收敛（2026-10-19）：<lg（1023px 及以下）一级平铺也收进汉堡
 *  抽屉（与 <md 同款 compact 形态）。实测 820–1024 宽下 5 个一级项
 *  flex-wrap 竖排成 5 行、顶栏高 259px，占掉首屏近 1/3——平板这个
 *  宽度段「横排排不下、竖排太占地」，一级菜单只由 lg+ 桌面平铺承载。 */
type NavItem = {
  href: string;
  label: string;
  sub?: { label: string; href: string; items: NavItem[] }[];
};
type NavGroup = { group: string; items: NavItem[] };

export function MainMenu({
  items,
  groups,
  moreLabel,
  ariaLabel,
}: {
  items: NavItem[];
  /** 「更多 ▾」收纳域分组；缺省则不渲染下拉 */
  groups?: NavGroup[];
  moreLabel?: string;
  ariaLabel: string;
}) {
  const pathname = usePathname();
  const sp = useSearchParams();
  const [moreOpen, setMoreOpen] = useState(false);
  const moreRef = useRef<HTMLLIElement>(null);
  // M1 小屏收敛：<md 隐藏一级平铺，全部入口收进「更多 ▾」（M2 抽屉的过渡兜底）。
  // SSR 首帧 fallback=false（桌面）：与 use-media 全站纪律一致，挂载后校正。
  const compact = useMediaQuery("(max-width: 1023px)", false);

  const pathActive = (p: string) =>
    p === "/"
      ? pathname === "/"
      : pathname === p || pathname.startsWith(`${p}/`);
  /** 当前 URL 是否精确携带该 href 的 query（键值全等，非子集）。
   *  2026-11-03 站长报「选资源-电影点亮四个按钮」：旧口径只查「href 的参数
   *  都在 URL 里」，子集匹配——category_id=1&official=0 会同时命中
   *  全部-电影（category_id 子集）/资源-全部（official 子集）/
   *  资源-更多分类…（overflow href 同 official）四条。全等口径下
   *  「URL 比菜单项多参数」不再算命中，同参组合唯一点亮。 */
  const queryActive = (href: string) => {
    const q = href.split("?")[1];
    if (!q) return false;
    const want = new URLSearchParams(q);
    // 菜单不关心的展示态/翻游标参数不参与比对：URL 带着它们时菜单条目
    // 仍应点亮（翻到第 3 页不代表离开了「资源-电影」频道）
    const ignore = new Set(["view", "limit", "cursor", "dir", "sort", "group"]);
    const cur = new URLSearchParams();
    for (const k of sp.keys()) {
      if (!ignore.has(k)) cur.set(k, sp.get(k) ?? "");
    }
    if (cur.size !== want.size) return false;
    for (const [k, v] of want) {
      if (cur.get(k) !== v) return false;
    }
    return true;
  };
  // 全部带参条目（含资源库下拉的三级叶子——MainMenu 渲染 sub 时也用
  // isActive）各自「认领」自己的路径：裸路径条目在任何带参条目命中时让位，
  // 否则资源库/官种/更多会一起点亮。
  const allLeaves = [
    ...items,
    ...items.flatMap((i) => i.sub?.flatMap((g) => g.items ?? []) ?? []),
    ...(groups ?? []).flatMap((g) => g.items),
  ];
  const claimedPaths = new Set(
    allLeaves
      .filter((i) => queryActive(i.href))
      .map((i) => i.href.split("?")[0]),
  );

  const isActive = (href: string) => {
    const [path, query] = href.split("?");
    if (!pathActive(path)) return false;
    if (query) return queryActive(href);
    return !claimedPaths.has(path);
  };
  // 资源库（带 sub 的一级）：任一叶子命中即父亮——频道页里顶栏一级入口
  // 保持「你在资源库里」的上下文（2026-11-03 三级下拉补的口径；此前裸
  // /torrents 在 ?category_id=1 让位给叶子后，一级反而不亮了）。
  // 叶子的 href 带查询串，直接复用 isActive 的全等判断。
  const hasActiveLeaf = (n: NavItem) =>
    !!n.sub &&
    n.sub.some((g) => (g.items ?? []).some((leaf) => isActive(leaf.href)));
  const moreActive = (groups ?? []).some((g) =>
    g.items.some((i) => isActive(i.href)),
  );
  // 小屏收敛态：一级条目并入「更多 ▾」首组，保证入口仍一步可达（M1 过渡）。
  const dropGroups = compact
    ? [{ group: ariaLabel, items }, ...(groups ?? [])]
    : (groups ?? []);

  useEffect(() => {
    if (!moreOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (moreRef.current && !moreRef.current.contains(e.target as Node))
        setMoreOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMoreOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [moreOpen]);

  return (
    <nav aria-label={ariaLabel}>
      <ul className="mainmenu">
        {items.map((n) => {
          if (compact) return null;
          const external = /^https?:\/\//i.test(n.href);
          return (
            <li
              key={n.href}
              className={n.sub ? "relative libmenu" : undefined}
            >
              {n.sub ? (
                // 资源库下拉（2026-10-31）：hover 展开（触屏点按，CSS 里
                // 有 @media (hover:none) 的 click 口径）；面板二级=全部/
                // 资源/官种三组，三级=全部+站型顶级分类叶子
                <>
                  <Link
                    href={n.href}
                    className="mainmenu-link"
                    data-active={
                      isActive(n.href) || hasActiveLeaf(n)
                        ? "true"
                        : undefined
                    }
                  >
                    {n.label} ▾
                  </Link>
                  <div className="mainmenu-drop mainmenu-drop--lib">
                    {n.sub.map((g) => (
                      <div key={g.label} className="mainmenu-drop__group">
                        <Link
                          href={g.href}
                          className="mainmenu-drop__title
                                     mainmenu-drop__title--link"
                        >
                          {g.label}
                        </Link>
                        <div className="mainmenu-drop__links">
                          {g.items.map((leaf) => (
                            <Link
                              key={leaf.href + leaf.label}
                              href={leaf.href}
                              className="mainmenu-drop__link"
                              data-active={
                                isActive(leaf.href) ? "true" : undefined
                              }
                            >
                              {leaf.label}
                            </Link>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              ) : external ? (
                <a
                  href={n.href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="mainmenu-link"
                >
                  {n.label}
                </a>
              ) : (
                <Link
                  href={n.href}
                  className="mainmenu-link"
                  data-active={isActive(n.href) ? "true" : undefined}
                >
                  {n.label}
                </Link>
              )}
            </li>
          );
        })}
        {(compact || (groups && groups.length > 0)) && (
          <li className="relative" ref={moreRef}>
            <button
              type="button"
              className="mainmenu-link"
              data-active={moreActive || moreOpen ? "true" : undefined}
              aria-expanded={moreOpen}
              aria-haspopup="true"
              onClick={() => setMoreOpen((v) => !v)}
            >
              {moreLabel}
            </button>
            {moreOpen && (
              <div className="mainmenu-drop">
                {dropGroups.map((g) => (
                  <div key={g.group} className="mainmenu-drop__group">
                    <p className="mainmenu-drop__title">{g.group}</p>
                    <div className="mainmenu-drop__links">
                      {g.items.map((i) => (
                        <Link
                          key={i.href}
                          href={i.href}
                          onClick={() => setMoreOpen(false)}
                          className="mainmenu-drop__link"
                          data-active={isActive(i.href) ? "true" : undefined}
                        >
                          {i.label}
                        </Link>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </li>
        )}
      </ul>
    </nav>
  );
}
