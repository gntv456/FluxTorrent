"use client";

import { useEffect, useState } from "react";
import type { Dict } from "@/i18n/zh-CN";

/** 帖子页右栏：赞/藏/回 三键 + 本文目录（2026-11-12 三栏改版）。
 *
 *  操作组的真身在头部操作行（关注/收藏/举报/编辑），此处是**锚点版**
 *  ——滚动到任意深度都能一键赞藏回，不用回顶部。三个动作：
 *  - 点赞：乐观更新调 /forums/posts/{首楼}/like（PostVoteBar 同口径），
 *    首楼 id 由服务端传入；
 *  - 收藏/回复：锚点跳到头部操作行（真动作在那里，不重复实现表单），
 *    回复落点直接对准底部回复框。
 *  本文目录：从正文 Markdown 标题（h2/h3）抽取（服务端传给 id 列表），
 *  点击平滑滚动 + 高亮当前节（IntersectionObserver，锚点条同款口径）。
 *  无标题的长帖目录块不渲染，不演空态。 */

export interface TocItem {
  id: string;
  text: string;
  level: number;
}

export function TopicSideActions({
  firstPostId,
  likes,
  liked,
  canLike,
  favHref,
  replyHref,
  toc,
  labels,
}: {
  firstPostId: number;
  likes: number;
  liked: boolean;
  /** 未登录/无权限时点赞钮禁用（头部操作行同门） */
  canLike: boolean;
  /** 收藏/回复锚点（页内 #id） */
  favHref: string;
  replyHref: string;
  toc: TocItem[];
  labels: Pick<
    Dict["forums3"],
    "sideLike" | "sideFav" | "sideReply" | "tocTitle" | "sideNoToc"
  >;
}) {
  return (
    <>
      <SideActions
        firstPostId={firstPostId}
        likes={likes}
        liked={liked}
        canLike={canLike}
        favHref={favHref}
        replyHref={replyHref}
        labels={labels}
      />
      {toc.length > 0 ? (
        <TopicToc items={toc} title={labels.tocTitle} />
      ) : null}
    </>
  );
}

function SideActions({
  firstPostId,
  likes,
  liked,
  canLike,
  favHref,
  replyHref,
  labels,
}: {
  firstPostId: number;
  likes: number;
  liked: boolean;
  canLike: boolean;
  favHref: string;
  replyHref: string;
  labels: Pick<Dict["forums3"], "sideLike" | "sideFav" | "sideReply">;
}) {
  const [on, setOn] = useState(liked);
  const [n, setN] = useState(likes);
  const [busy, setBusy] = useState(false);

  async function toggle() {
    if (busy || !canLike) return;
    setBusy(true);
    const next = !on;
    setOn(next);
    setN((x) => x + (next ? 1 : -1));
    try {
      const { api } = await import("@/lib/api-client");
      const r = next
        ? await api.post<{ likes: number; liked: boolean }>(
            `/api/v1/forums/posts/${firstPostId}/like`,
          )
        : await api.del<{ likes: number; liked: boolean }>(
            `/api/v1/forums/posts/${firstPostId}/like`,
          );
      if (r) {
        setOn(r.liked);
        setN(r.likes);
      }
    } catch {
      setOn(!next);
      setN((x) => x + (next ? -1 : 1));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="side-card" aria-label={labels.sideLike}>
      <div className="tp-side__acts">
        <button
          type="button"
          onClick={toggle}
          disabled={busy || !canLike}
          aria-pressed={on}
          className={`tp-side__act tp-side__act--like${on ? " is-on" : ""}`}
        >
          <span aria-hidden>👍</span>
          <b className="num">{n}</b>
          <span>{labels.sideLike}</span>
        </button>
        <a href={favHref} className="tp-side__act">
          <span aria-hidden>⭐</span>
          <span>{labels.sideFav}</span>
        </a>
        <a href={replyHref} className="tp-side__act">
          <span aria-hidden>💬</span>
          <span>{labels.sideReply}</span>
        </a>
      </div>
    </section>
  );
}

function TopicToc({ items, title }: { items: TocItem[]; title: string }) {
  const [on, setOn] = useState<string | null>(null);
  useEffect(() => {
    const els = items
      .map((it) => document.getElementById(it.id))
      .filter((e): e is HTMLElement => !!e);
    if (els.length === 0) return;
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          const first = items.find((it) => {
            const el = document.getElementById(it.id);
            return el && els.includes(el) && e.target === el;
          });
          if (first && e.isIntersecting) setOn(first.id);
        }
      },
      { rootMargin: "-100px 0px -55% 0px" },
    );
    els.forEach((el) => io.observe(el));
    return () => io.disconnect();
  }, [items]);

  return (
    <nav className="side-card tp-toc" aria-label={title}>
      <h2 className="side-card__title">{title}</h2>
      <ul className="tp-toc__list">
        {items.map((it) => (
          <li key={it.id} className={it.level >= 3 ? "tp-toc__sub" : undefined}>
            <a
              href={`#${it.id}`}
              className={`tp-toc__a${on === it.id ? " is-on" : ""}`}
              onClick={(e) => {
                e.preventDefault();
                const el = document.getElementById(it.id);
                if (!el) return;
                el.scrollIntoView({ behavior: "smooth", block: "start" });
                setOn(it.id);
              }}
            >
              {it.text}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}
