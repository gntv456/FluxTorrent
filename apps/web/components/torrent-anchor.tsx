"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useMediaQuery } from "@/lib/hooks/use-media";
import type { Dict } from "@/i18n/zh-CN";

/** 移动端锚点条（设计稿 §5/§6 td-anchor）：sticky 吸顶横向 Tab——
 *  简介 / 文件 / 字幕 / 评论，点击平滑滚动至对应折叠卡并自动展开；
 *  滚动时 IntersectionObserver 高亮当前区块（可见集取文档序最靠前）。
 *  仅 ≤768 渲染（本地完善版样图 MD 帧含锚点条；桌面导航角色由右信息栏
 *  承担）；SSR 首帧桌面态返回 null，与 FoldMobile 同 hydration 纪律
 *  （宁可首帧少渲染一版，不冒服务端/客户端 markup 不一致的风险）。
 *  断点与共享 useIsCompact（<767，喂底 Tab/吸底条一大票组件）解耦：
 *  M6.3 定过 768 整档保留头部操作区，锚点是附加导航不重回该死区。
 *
 *  RSC 瘦身（2026-11-11）：本组件是 client 边界，此前收整本 Dict——
 *  整本字典被序列化进 /torrent/{id} flight payload（131KB，占 60%）。
 *  锚点条只用 5 枚文案，改为切片 prop（调用侧传 pick 出的对象）。 */
export type AnchorDict = Pick<
  Dict["torrent"],
  "descrTitle" | "filesTitle" | "commentsTitle" | "anchorNav"
> &
  Pick<Dict["subtitles"], "panelTitle">;

export function TorrentAnchorBar({
  descr,
  files,
  subtitles,
  comments,
  dict,
}: {
  /** 简介折叠卡是否渲染（sectOn + 有正文） */
  descr: boolean;
  /** 文件数；null = 文件卡不渲染（无文件或节被隐藏） */
  files: number | null;
  /** 字幕面板是否渲染 */
  subtitles: boolean;
  /** 评论数；null = 评论区隐藏（徽标仅在有评论时显示） */
  comments: number | null;
  dict: AnchorDict;
}) {
  // 本地完善版口径 ≤768（样图 MD 帧）：不借共享 useIsCompact（<767）
  const compact = useMediaQuery("(max-width: 768px)", false);
  // items 组装在组件内（page.tsx 只传标志位）：不动详情页行数基线
  const items = useMemo(() => {
    const { descrTitle, filesTitle, commentsTitle } = dict;
    const subTitle = dict.panelTitle;
    const out: { id: string; label: string; count?: number }[] = [];
    if (descr) out.push({ id: "td-sec-descr", label: descrTitle });
    if (files) out.push({ id: "td-files", label: filesTitle, count: files });
    if (subtitles) out.push({ id: "td-sec-subtitles", label: subTitle });
    if (comments !== null) {
      out.push({
        id: "td-sec-comments",
        label: commentsTitle.replace(/\s*\(?\{n\}\)?/, ""),
        count: comments,
      });
    }
    return out;
  }, [descr, files, subtitles, comments, dict]);

  const [on, setOn] = useState<string | null>(null);
  const vis = useRef<Set<string>>(new Set());

  // 滚动高亮：rootMargin 顶部 -120px 避开吸顶条（72 顶栏 + ~42 条高），
  // 底部 -40% 让首屏下方区块也进判定；items 顺序即文档序
  useEffect(() => {
    if (!compact || items.length === 0) return;
    vis.current = new Set();
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) vis.current.add(e.target.id);
          else vis.current.delete(e.target.id);
        }
        const first = items.find((i) => vis.current.has(i.id));
        setOn(first ? first.id : null);
      },
      { rootMargin: "-120px 0px -40% 0px" },
    );
    for (const it of items) {
      const el = document.getElementById(it.id);
      if (el) io.observe(el);
    }
    return () => io.disconnect();
  }, [compact, items]);

  // 点击：收起态的折叠卡先展开（details 的 open 由 React 以
  // open={!compact} 控制但无本地 state，直接置 DOM 属性即生效，
  // 断点翻转时才会被重置），再滚动到区块；落点避让见 base.css
  // .td-fold 的 scroll-margin-top（<768 加大到 124px）。
  // 滚动健壮性（2026-11-12 移动端实测）：嵌入式 webview 的 smooth
  // 通道会静默不执行（真实滚动 0px；后台节流下 rAF 也不回调，故不
  // 能用 rAF 延后）——起滚后采样，没动就 instant 兜底；真实浏览器
  // 的平滑动画不受影响。
  const jump = (id: string) => {
    const el = document.getElementById(id);
    if (!el) return;
    if (el instanceof HTMLDetailsElement && !el.open) el.open = true;
    setOn(id);
    const before = scrollY;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    setTimeout(() => {
      if (Math.abs(scrollY - before) < 4) {
        el.scrollIntoView({ behavior: "auto", block: "start" });
      }
    }, 350);
    // 二段兜底：有的 webview 把 smooth 掐死在半途（滚了但没到落点），
    // 700ms 时按 scroll-margin 复核，偏差大就 instant 校正到位；
    // 真实浏览器 smooth 完成后落点≈0，不会触发。
    setTimeout(() => {
      const margin =
        parseFloat(getComputedStyle(el).scrollMarginTop || "0") || 0;
      const offset = el.getBoundingClientRect().top - margin;
      if (Math.abs(offset) > 40) {
        el.scrollIntoView({ behavior: "auto", block: "start" });
      }
    }, 700);
  };

  if (!compact || items.length === 0) return null;
  return (
    <nav className="td-anchor" aria-label={dict.anchorNav}>
      {items.map((it) => (
        <button
          key={it.id}
          type="button"
          className={`td-anchor__tab${on === it.id ? " is-on" : ""}`}
          aria-current={on === it.id ? "true" : undefined}
          onClick={() => jump(it.id)}
        >
          {it.label}
          {it.count ? (
            <span className="td-anchor__num num">{it.count}</span>
          ) : null}
        </button>
      ))}
    </nav>
  );
}
