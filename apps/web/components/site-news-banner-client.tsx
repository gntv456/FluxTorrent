"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";

/** 全站公告横幅（0503）：news 此前只在首页渲染——深逛 /torrents 等页的
 *  用户对新公告（规则变更/封禁公示/活动）零感知。
 *
 *  2026-11-20 五问评审：原注释写"横幅可关闭、localStorage 记已读 id"，
 *  但实现里**根本没有关闭按钮**——用户被强制反复看同一条公告。
 *  本版补上关闭钮，按公告 id 记入 localStorage，仅对当前这条生效。 */
export function SiteNewsBanner({
  top,
}: {
  top: { id: number; title: string } | null;
}) {
  const { dict } = useI18n();
  const [dismissed, setDismissed] = useState(true); // SSR 不渲染，等 localStorage 判定
  useEffect(() => {
    if (!top) return;
    try {
      setDismissed(
        localStorage.getItem("site-news-dismissed") === String(top.id),
      );
    } catch {
      setDismissed(false);
    }
  }, [top]);
  if (!top || dismissed) return null;
  return (
    <div
      role="status"
      className="site-news-banner flex items-center justify-center
        gap-2 border-b border-[var(--line)] bg-[var(--surface-sunken)]
        px-4 py-1.5 text-center text-sm"
      data-news-id={top.id}
    >
      <Badge tone="warn">{dict.siteNews.label}</Badge>
      <Link
        href="/"
        className="text-link truncate min-h-[44px] inline-flex items-center"
      >
        {top.title}
      </Link>
      <button
        type="button"
        className="min-h-11 px-2 text-sub"
        aria-label={dict.review.newsDismiss}
        title={dict.review.newsDismiss}
        onClick={() => {
          try {
            localStorage.setItem("site-news-dismissed", String(top.id));
          } catch {
            /* 隐私模式：本次会话内仍生效 */
          }
          setDismissed(true);
        }}
      >
        ✕
      </button>
    </div>
  );
}
