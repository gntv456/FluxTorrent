"use client";

import { useI18n } from "@/i18n/client";

/** 首页公告（社区新鲜事）的手机端头条卡。
 *
 *  现状：桌面那套「巨幅海报 + 公告全展开」在手机上占 630px，且排第 2 块。
 *  这里改成一条 176px 的头条卡：斜纹头（复用站方 brand-panel__head--ribbon
 *  的斜纹语言）+ 徽标 + 大标题 + 两行摘要 + 剩余公告计数，点标题开正文弹窗
 *  （正文弹窗复用现成的 HomeNewsModal，Esc / 焦点圈定都有）。 */

export interface HomeNewsItem {
  id: number;
  title: string;
  body: string;
  badge: string;
  date: string;
}

export function MobileNewsTop({
  news,
  onOpen,
}: {
  news: HomeNewsItem[];
  /** 打开公告正文弹窗（父级持 modal 状态） */
  onOpen: (id: number) => void;
}) {
  const { dict } = useI18n();
  const t = dict.home2;
  if (news.length === 0) return null;
  const head = news[0];
  // 正文可能带 HTML（0190 允许内嵌视频），摘要只取纯文本
  const plain = head.body.replace(/<[^>]+>/g, "").trim();
  return (
    <section className="mhome-news-top md:hidden">
      <div className="mhome-news-top__head">
        <h3>
          <span aria-hidden="true">📣</span> {t.newsTitle}
        </h3>
        <span>{t.allNews} ›</span>
      </div>
      <button
        type="button"
        className="mhome-news-top__body"
        onClick={() => onOpen(head.id)}
      >
        <span className="mhome-news-top__badge">
          {head.badge || t.newsBadge}
        </span>
        <b className="mhome-news-top__title">{head.title}</b>
        {plain ? <p className="mhome-news-top__sum">{plain}</p> : null}
      </button>
      <div className="mhome-news-top__foot">
        <span className="mhome-news-top__unread">
          <i aria-hidden="true" />
          {t.newsMore.replace("{n}", String(news.length - 1))}
        </span>
        <span>{head.date}</span>
      </div>
    </section>
  );
}
