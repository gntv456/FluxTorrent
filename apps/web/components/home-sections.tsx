"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ShoutBox } from "@/components/shout-box";
import { FunBox } from "@/components/fun-box";
import { AttendanceCard } from "@/components/home-attendance";
import { SiteDataCard } from "@/components/home-site-data";
import { ResourceStatsPanel } from "@/components/home-resource-stats";
import { effectiveSpan, parseHomeLayout } from "@/components/home-layout";
import type { HomeData } from "@/components/home-data";

/** 首页板块（复刻参考站 index.php）：
 *  社区新鲜事（头条+列表+公告弹窗）· 签到得魔力日历 · 新增资源统计（30 天堆叠柱状图）
 *  · 站点数据三列 · 幸运大转盘流水 · 免责条款 + 友情链接。
 *  拆出（300 门禁）：排版解析 home-layout.tsx、数据契约 home-data.ts、
 *  签到日历 home-attendance.tsx、站点数据 home-site-data.tsx、
 *  资源统计 home-resource-stats.tsx、补签弹层 home-resub-button.tsx。 */

export function HomeSections() {
  const { dict, currency } = useI18n();
  const t = dict.home2;
  const [data, setData] = useState<HomeData | null>(null);
  const [err, setErr] = useState(false);
  const [modal, setModal] = useState<number | null>(null);
  const [checkinBusy, setCheckinBusy] = useState(false);
  const [checkinMsg, setCheckinMsg] = useState<string | null>(null);

  const load = () => {
    api
      .get<HomeData>("/api/v1/home")
      .then(setData)
      .catch(() => setErr(true));
  };
  useEffect(load, []);

  async function checkin() {
    if (checkinBusy) return;
    setCheckinBusy(true);
    try {
      const r = await api.post<{ reward: number; streak: number }>(
        "/api/v1/attendance/checkin",
      );
      setCheckinMsg(
        dict.my.checkinOk
          .replace("{reward}", String(r.reward))
          .replace("{streak}", String(r.streak))
          .replace("{magic}", currency),
      );
      load();
    } catch {
      setCheckinMsg(dict.home2.checkinFail);
    } finally {
      setCheckinBusy(false);
    }
  }

  if (err) return <p className="text-sm text-sub">{dict.common.loadFailed}</p>;
  if (!data) return <p className="text-sm text-sub">{dict.my.loading}</p>;

  const [headline, ...rest] = data.news;
  const headlineBodyPlain =
    headline?.body.replace(/<[^>]+>/g, "").slice(0, 160) ?? "";

  const layout = parseHomeLayout(data.home_layout);
  // 闭包内 TS 判窄失效：固化非空引用供 renderSection 使用
  const home = data;

  /** 单板块渲染（0089 排版）：key 与后端 HOME_SECTION_KEYS 一一对应 */
  function renderSection(key: string): React.ReactNode {
    switch (key) {
      case "news":
        return (
          <section className="baozi-panel home-news">
            <header className="baozi-panel__head baozi-panel__head--ribbon">
              <h1>
                <span aria-hidden="true">📣</span> {t.newsTitle}
              </h1>
            </header>
            <div className="home-news__body">
              <div className="home-news__poster" aria-hidden="true">
                {dict.common.brand}
              </div>
              <div className="home-news__content">
                {headline && (
                  <article className="home-news__summary">
                    <strong>{headline.title}</strong>
                    <p>{headlineBodyPlain}…</p>
                    <div className="home-news__summary-footer">
                      <button
                        type="button"
                        className="baozi-button"
                        onClick={() => setModal(headline.id)}
                      >
                        {t.viewNews}
                      </button>
                    </div>
                  </article>
                )}
                <div className="home-news__list" aria-label={t.moreNews}>
                  {rest.map((n) => (
                    <button
                      key={n.id}
                      type="button"
                      className="home-news__item"
                      onClick={() => setModal(n.id)}
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
      case "attendance":
        return (
          <AttendanceCard
            home={home}
            t={t}
            dict={dict}
            checkin={checkin}
            checkinBusy={checkinBusy}
            checkinMsg={checkinMsg}
          />
        );
      case "shoutbox":
        return <ShoutBox />;
      case "funbox":
        return <FunBox embedded />;
      case "resource_stats":
        return <ResourceStatsPanel series={home.resource_stats} t={t} />;
      case "site_data":
        return <SiteDataCard home={home} t={t} dict={dict} />;
      case "lucky_draw":
        return (
          <aside
            className="baozi-panel home-lucky-draw"
            aria-label={t.luckyTitle}
          >
            <header className="baozi-panel__head">
              <h2>
                <span aria-hidden="true">🎰</span> {t.luckyTitle}
              </h2>
              <Link href="/games">{t.goDraw}</Link>
            </header>
            <ul className="home-lucky-draw__list">
              {home.lucky_draw.map((l, i) => (
                <li key={i}>
                  <b className="rainbow">{l.user}</b> {t.got}{" "}
                  {dict.common.spark.replace("{magic}", currency)} {l.amount}
                </li>
              ))}
              {home.lucky_draw.length === 0 && (
                <li className="text-sub">{t.luckyEmpty}</li>
              )}
            </ul>
          </aside>
        );
      case "links":
        return (
          <div className="home-native-modules">
            <h2>{t.disclaimerTitle}</h2>
            <p className="home-native-modules__text">{t.disclaimerBody}</p>
            <h2>
              {t.linksTitle}
              <small>
                {" "}
                - [<Link href="/links/apply">{t.applyLink}</Link>]
              </small>
            </h2>
            <p className="home-native-modules__text">
              {home.friend_links.map((l) => (
                <a
                  key={l.url}
                  href={l.url}
                  title={l.title ?? l.name}
                  target="_blank"
                  rel="noreferrer"
                >
                  {l.name}
                </a>
              ))}
            </p>
          </div>
        );
      default:
        return null;
    }
  }

  const isCustom = home.home_layout?.trim() ? true : false;
  return (
    <div className={isCustom ? "home-stack home-stack--custom" : "home-stack"}>
      {/* ==== 排版驱动（0089）：按站长配置顺序/宽度渲染；空配置 = 默认布局 ==== */}
      {layout.map((item) => {
        const node = renderSection(item.key);
        if (!node) return null;
        return (
          <div
            key={item.key}
            className={`home-cell home-cell--span-${effectiveSpan(item)}`}
          >
            {node}
          </div>
        );
      })}

      {/* ==== 公告弹窗 ==== */}
      {modal !== null && (
        <dialog
          className="home-news-modal"
          open
          onClick={(e) => e.target === e.currentTarget && setModal(null)}
        >
          <section className="home-news-modal__panel">
            <header className="home-news-modal__head">
              <div>
                <span>{t.newsBadge}</span>
                <h2>{data.news.find((n) => n.id === modal)?.title}</h2>
              </div>
              <button
                type="button"
                aria-label={t.closeModal}
                title={t.closeModal}
                onClick={() => setModal(null)}
              >
                ×
              </button>
            </header>
            <div
              className="home-news-modal__body"
              dangerouslySetInnerHTML={{
                __html: data.news.find((n) => n.id === modal)?.body ?? "",
              }}
            />
          </section>
        </dialog>
      )}
    </div>
  );
}
