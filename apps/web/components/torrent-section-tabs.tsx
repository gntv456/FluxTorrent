"use client";

import { useEffect, useState } from "react";
import { useMediaQuery } from "@/lib/hooks/use-media";
import { FoldMobile } from "@/components/fold-mobile";

/** 详情页主栏标签组（2026-11-13 省高改版）：左主栏十来块折叠卡平铺、
 *  全收起时折叠头也要吃 ~700px——改为规格条/简介之外的段落收进一个
 *  标签组卡片，tab 切换省高度。
 *
 *  - 桌面（>768）：胶囊 tab 条（视觉与移动端锚点条同语言）+ panel。
 *    挂载语义对齐 Fold 门卫：只有「初始激活或点过」的 tab 才挂载内容，
 *    切走不卸载（peers 快照/字幕表单/评论分页状态不丢），未访问的
 *    tab 零请求零 DOM；多 tab 都看过后与展开过的折叠卡同价。
 *  - ≤768：回落平铺折叠（M5.8 降噪形态，与本组件出现之前一致），
 *    每块由 FoldMobile 包裹、id 落在 details 上——移动端锚点条的
 *    定位/自动展开机制原样工作。
 *  - 旧锚点兼容：panel 容器沿用各段历史 id（td-files /
 *    td-sec-subtitles…）。桌面端 hash 命中某 tab 时自动切换激活
 *    （侧栏文件卡「查看全部 {n} 个文件」、站外旧链都不落空）。
 *  - SSR 首帧按桌面标签组渲染（use-media 纪律：宁可首帧多渲染一版
 *    桌面，不冒 hydration markup 不一致的风险）。 */

export interface SectionTab {
  /** 锚点/tab id：桌面落在 panel 容器、移动端落在 details 上 */
  id: string;
  label: string;
  /** 计数徽标（文件数/评论数…）；0/缺省不显示 */
  count?: number;
  /** 裸内容（无折叠外壳）：桌面进 panel，移动端由本组件包 FoldMobile */
  node: React.ReactNode;
}

export function TorrentSectionTabs({
  items,
  navLabel,
}: {
  items: SectionTab[];
  /** tab 条 aria-label（复用锚点条「页面导航」文案） */
  navLabel: string;
}) {
  // 与移动端锚点条（td-anchor 的 768 档）同断点：≤768 平铺回落
  const compact = useMediaQuery("(max-width: 768px)", false);
  const [active, setActive] = useState(items[0]?.id ?? "");
  // 挂载过的 tab 集合（Fold revealed 语义）：首访问才挂载、切回不重拉
  const [visited, setVisited] = useState<ReadonlySet<string>>(
    () => new Set(items[0] ? [items[0].id] : []),
  );

  // 旧锚点兼容（桌面）：#td-files 等命中即切 tab 并挂载。
  // hashchange 覆盖侧栏「查看全部」与站外旧链；初始 hash 同样生效。
  useEffect(() => {
    const apply = () => {
      const h = decodeURIComponent(location.hash.slice(1));
      if (h && items.some((it) => it.id === h)) {
        setActive(h);
        setVisited((prev) => (prev.has(h) ? prev : new Set(prev).add(h)));
      }
    };
    apply();
    window.addEventListener("hashchange", apply);
    return () => window.removeEventListener("hashchange", apply);
  }, [items]);

  // router.refresh 后 items 可能变（段落显隐调整）：active 失效回落首 tab
  const cur = items.some((it) => it.id === active)
    ? active
    : (items[0]?.id ?? "");

  // ≤768 平铺（M5.8）：每块独立折叠卡，id 在 details 上供锚点条定位
  if (compact) {
    return (
      <>
        {items.map((it) => (
          <FoldMobile key={it.id} title={it.label} count={it.count} id={it.id}>
            {it.node}
          </FoldMobile>
        ))}
      </>
    );
  }

  if (items.length === 0) return null;
  return (
    <section className="td-tabs">
      <nav className="td-tabs__bar" aria-label={navLabel}>
        {items.map((it) => (
          <button
            key={it.id}
            type="button"
            role="tab"
            aria-selected={cur === it.id}
            aria-controls={`td-tabpanel-${it.id}`}
            className={`td-tabs__tab${cur === it.id ? " is-on" : ""}`}
            onClick={() => {
              setActive(it.id);
              setVisited((prev) =>
                prev.has(it.id) ? prev : new Set(prev).add(it.id),
              );
            }}
          >
            {it.label}
            {it.count ? (
              <span className="td-tabs__num num">{it.count}</span>
            ) : null}
          </button>
        ))}
      </nav>
      {items.map((it) => (
        <div
          key={it.id}
          id={it.id}
          role="tabpanel"
          hidden={cur !== it.id}
          className="td-tabs__panel"
        >
          {cur === it.id || visited.has(it.id) ? it.node : null}
        </div>
      ))}
    </section>
  );
}
