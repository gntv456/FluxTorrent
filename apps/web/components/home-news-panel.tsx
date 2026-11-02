"use client";

import { useI18n } from "@/i18n/client";
import { HomeNewsPoster } from "@/components/home-news-poster";

/** 首页「社区新鲜事」桌面面板（从 home-sections.tsx 按域拆出，给它腾行数）。
 *
 *  手机端另有紧凑形态（mobile-home-news-top.tsx 的头条卡，置顶在首屏），
 *  这一支是桌面的原样：斜纹头 + 海报位 + 头条摘要 + 公告列表。 */

export interface NewsItem {
  id: number;
  title: string;
  body: string;
  badge: string;
  date: string;
}

export function HomeNewsPanel({
  items,
  onOpen,
}: {
  items: NewsItem[];
  /** 打开公告正文弹窗 */
  onOpen: (id: number) => void;
}) {
  const { dict } = useI18n();
  const t = dict.home2;
  const [headline, ...rest] = items;
  const plain = headline?.body.replace(/<[^>]+>/g, "").slice(0, 160) ?? "";
  return (
    <section className="brand-panel home-news">
      <header className="brand-panel__head brand-panel__head--ribbon">
        <h2>
          <span aria-hidden="true">📣</span> {t.newsTitle}
        </h2>
      </header>
      <div className="home-news__body">
        <HomeNewsPoster
          brand={dict.common.brand}
          items={rest.slice(0, 3).map((n) => ({ id: n.id, title: n.title }))}
        />
        <div className="home-news__content">
          {headline && (
            <article className="home-news__summary">
              <strong>{headline.title}</strong>
              <p>{plain}…</p>
              <div className="home-news__summary-footer">
                <button
                  type="button"
                  className="brand-button"
                  onClick={() => onOpen(headline.id)}
                >
                  {t.viewNews}
                </button>
              </div>
            </article>
          )}
          <div aria-label={t.moreNews} className="home-news__list">
            {rest.map((n) => (
              <button
                key={n.id}
                type="button"
                className="home-news__item"
                onClick={() => onOpen(n.id)}
              >
                <span className="home-news__badge">{n.badge}</span>
                <strong>{n.title}</strong>
                <time>{n.date}</time>
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
