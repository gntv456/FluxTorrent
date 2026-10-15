"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useIsCompact } from "@/lib/hooks/use-media";
import type { Dict } from "@/i18n/zh-CN";

/** 移动端锚点条（设计稿 §5/§6 td-anchor）：sticky 吸顶横向 Tab——
 *  简介 / 文件 / 字幕 / 评论，点击平滑滚动至对应折叠卡并自动展开；
 *  滚动时 IntersectionObserver 高亮当前区块（可见集取文档序最靠前）。
 *  仅 <768 渲染（桌面导航角色由右信息栏承担）；SSR 首帧桌面态返回
 *  null，与 FoldMobile 同 hydration 纪律（宁可首帧少渲染一版，不冒
 *  服务端/客户端 markup 不一致的风险）。 */
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
  dict: Dict;
}) {
  const compact = useIsCompact();
  // items 组装在组件内（page.tsx 只传标志位）：不动详情页行数基线
  const items = useMemo(() => {
    const { descrTitle, filesTitle, commentsTitle } = dict.torrent;
    const subTitle = dict.subtitles.panelTitle;
    const out: { id: string; label: string; count?: number }[] = [];
    if (descr) out.push({ id: "td-sec-descr", label: descrTitle });
    if (files) out.push({ id: "td-files", label: filesTitle, count: files });
    if (subtitles) out.push({ id: "td-sec-subtitles", label: subTitle });
    if (comments !== null) {
      out.push({
        id: "td-sec-comments",
        label: commentsTitle.replace(/\s*\{n\}/, ""),
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
  // 断点翻转时才会被重置），再平滑滚动；落点避让见 base.css
  // .td-fold 的 scroll-margin-top（<768 加大到 124px）
  const jump = (id: string) => {
    const el = document.getElementById(id);
    if (!el) return;
    if (el instanceof HTMLDetailsElement && !el.open) el.open = true;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    setOn(id);
  };

  if (!compact || items.length === 0) return null;
  return (
    <nav className="td-anchor" aria-label={dict.torrent.anchorNav}>
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
