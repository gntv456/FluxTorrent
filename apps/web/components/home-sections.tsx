"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ShoutBox } from "@/components/shout-box";
import { FunBox } from "@/components/fun-box";

/** 首页板块（复刻参考站 index.php）：
 *  社区新鲜事（头条+列表+公告弹窗）· 签到得魔力日历 · 新增资源统计（30 天堆叠柱状图）
 *  · 站点数据三列 · 幸运大转盘流水 · 免责条款 + 友情链接 */

interface HomeData {
  news: { id: number; title: string; body: string; badge: string; date: string }[];
  attendance: {
    month: string;
    streak: number;
    total_days: number;
    checked_today: boolean;
    calendar: { date: string; day: number; done: boolean; reward: number }[];
    makeup_cards?: number;
  };
  resource_stats: {
    today: number;
    avg7: number;
    total30: number;
    series: { date: string; ordinary: number; official: number; total: number }[];
  };
  site_data: {
    users: number;
    torrents: number;
    peers: number;
    seeders: number;
    leechers: number;
    warned: number;
    banned: number;
    unverified: number;
    total_upload: number;
    total_download: number;
    total_size: number;
  };
  lucky_draw: { user: string; kind: string; amount: number }[];
  friend_links: { name: string; url: string; title: string | null }[];
  /** 首页排版（0089）：JSON 数组字符串或空串（空 = 默认布局） */
  home_layout?: string;
}

/** 首页板块排版（0089）：
 *  site_settings.home_layout 由站长在后台编辑——板块键有序数组 + 宽度档（span：
 *  1=1/3、2=2/3、3=整行、0/缺省=按板块推荐档）。空/非法配置回退 DEFAULT_LAYOUT，
 *  即原有排版（新鲜事+签到 → 聊天室+趣味盒 → 资源统计 → 站点数据+转盘 → 免责友链）。 */
export interface HomeLayoutItem {
  key: string;
  span: number;
}

export const HOME_SECTION_KEYS = [
  "news",
  "attendance",
  "shoutbox",
  "funbox",
  "resource_stats",
  "site_data",
  "lucky_draw",
  "links",
  "latest",
] as const;

export const DEFAULT_HOME_LAYOUT: HomeLayoutItem[] = [
  { key: "news", span: 0 },
  { key: "attendance", span: 0 },
  { key: "shoutbox", span: 0 },
  { key: "funbox", span: 0 },
  { key: "resource_stats", span: 3 },
  { key: "site_data", span: 0 },
  { key: "lucky_draw", span: 0 },
  { key: "links", span: 3 },
];

/** 推荐宽度档：未显式指定 span 的板块按此渲染（保持默认视觉） */
const RECOMMENDED_SPAN: Record<string, number> = {
  news: 2,
  attendance: 1,
  shoutbox: 2,
  funbox: 1,
  resource_stats: 3,
  site_data: 2,
  lucky_draw: 1,
  links: 3,
  latest: 3,
};

/** 解析后端 home_layout：结构/键非法整体回退默认（后台保存端点已强校验，
 *  这里兜底手改库或历史脏数据） */
export function parseHomeLayout(raw: string | undefined | null): HomeLayoutItem[] {
  if (!raw || !raw.trim()) return DEFAULT_HOME_LAYOUT;
  try {
    const arr = JSON.parse(raw) as { key?: string; span?: number }[];
    if (!Array.isArray(arr) || arr.length === 0) return DEFAULT_HOME_LAYOUT;
    const items: HomeLayoutItem[] = [];
    const seen = new Set<string>();
    for (const it of arr) {
      if (!it?.key || typeof it.key !== "string") return DEFAULT_HOME_LAYOUT;
      if (!(HOME_SECTION_KEYS as readonly string[]).includes(it.key)) return DEFAULT_HOME_LAYOUT;
      if (seen.has(it.key)) return DEFAULT_HOME_LAYOUT;
      seen.add(it.key);
      const span = typeof it.span === "number" && [0, 1, 2, 3].includes(it.span) ? it.span : 0;
      items.push({ key: it.key, span });
    }
    return items;
  } catch {
    return DEFAULT_HOME_LAYOUT;
  }
}

/** 板块实际宽度档：显式 span 优先，0 = 推荐档 */
export function effectiveSpan(item: HomeLayoutItem): number {
  return item.span || RECOMMENDED_SPAN[item.key] || 3;
}

function fmtBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(3)} ${units[i]}`;
}

/** 补签卡按钮（0066 修复）：持有数徽标 + 过去 7 天日期选择弹层 + 明确错误提示 */
function ResubButton({ cards }: { cards: number }) {
  const { dict, currency } = useI18n();
  const t = dict.attResub2 ?? { btn: "补签" } as { btn?: string };
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  // 可补日期：过去 7 天内（YYYY-MM-DD），按近到远
  const days = Array.from({ length: 7 }, (_, i) => {
    const d = new Date(Date.now() - (i + 1) * 86400e3);
    return d.toISOString().slice(0, 10);
  });
  async function doResub(date: string) {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ cards_left: number }>("/api/v1/attendance/resub", {
        target_date: date,
        idempotency_key: `resub-web-${date}-${crypto.randomUUID()}`,
      });
      setMsg(`${dict.attResub2?.ok ?? "补签成功"}（剩余补签卡 ${r.cards_left}）`);
      setTimeout(() => window.location.reload(), 800);
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "补签失败");
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <button
        type="button"
        className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold text-sub disabled:opacity-50"
        title={dict.attResub2?.note ?? "消耗补签卡（商店购买或管理发放），补过去 7 天内漏签"}
        disabled={busy}
        onClick={() => setOpen(!open)}
      >
        {t.btn ?? "补签"}
        <span className={`ml-1 rounded-full px-1.5 text-[10px] ${cards > 0 ? "bg-mint/30 text-ink" : "bg-coral/20 text-danger"}`}>
          ×{cards}
        </span>
      </button>
      {open && (
        <div className="fixed inset-0 z-30 flex items-center justify-center bg-black/30 p-3" role="dialog" aria-modal="true" onClick={() => setOpen(false)}>
          <div className="w-full max-w-sm rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]" onClick={(e) => e.stopPropagation()}>
            <h3 className="mb-1 text-sm font-bold">{t.btn ?? "补签"}（持有 {cards} 张）</h3>
            <p className="mb-3 text-xs text-sub">{dict.attResub2?.note ?? "消耗补签卡，补过去 7 天内漏签"}</p>
            {cards === 0 ? (
              <p className="rounded-[var(--r-md)] bg-sun/20 p-3 text-xs text-ink">
                暂无可用补签卡：可到<a href="/shop" className="font-bold text-sky">{currency}商店</a>购买，或等管理发放。
              </p>
            ) : (
              <div className="flex flex-wrap gap-2">
                {days.map((d) => (
                  <button
                    key={d}
                    disabled={busy}
                    onClick={() => doResub(d)}
                    className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold disabled:opacity-50 hover:border-sky"
                  >
                    {d.slice(5)}
                  </button>
                ))}
              </div>
            )}
            {msg && <p className="mt-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink">{msg}</p>}
            <button className="mt-3 min-h-[32px] rounded-full border border-line px-3 text-xs" onClick={() => setOpen(false)}>关闭</button>
          </div>
        </div>
      )}
    </>
  );
}

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
      const r = await api.post<{ reward: number; streak: number }>("/api/v1/attendance/checkin");
      setCheckinMsg(dict.my.checkinOk.replace("{reward}", String(r.reward)).replace("{streak}", String(r.streak)));
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
  const headlineBodyPlain = headline?.body.replace(/<[^>]+>/g, "").slice(0, 160) ?? "";

  // 周一开头对齐：本月 1 号之前补空位
  const firstDate = new Date(data.attendance.calendar[0]?.date ?? Date.now());
  const leadingBlanks = (firstDate.getDay() + 6) % 7; // 周一=0

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
          <aside className="baozi-panel attendance-card">
            <header className="baozi-panel__head">
              <h2>
                <span aria-hidden="true">📅</span> {t.attendanceTitle}
              </h2>
              <span className="flex items-center gap-2">
                {home.attendance.checked_today ? (
                  <span className="attendance-done">{t.attended}</span>
                ) : (
                  <button type="button" className="baozi-button" onClick={checkin} disabled={checkinBusy}>
                    {checkinBusy ? dict.my.checkinBusy : t.checkinNow}
                  </button>
                )}
                <ResubButton cards={home.attendance.makeup_cards ?? 0} />
              </span>
            </header>
            {checkinMsg && <p className="attendance-msg">{checkinMsg}</p>}
            <div className="attendance-card__summary">
              <div>
                <strong>{home.attendance.month}</strong>
                <span>
                  {t.streak} {home.attendance.streak} {dict.usercp.days}
                </span>
              </div>
              <div>
                <strong>{home.attendance.total_days}</strong>
                <span>{t.totalDays}</span>
              </div>
            </div>
            <div className="attendance-calendar" aria-label={`${home.attendance.month}${t.calendar}`}>
              {t.weekdays.map((w) => (
                <span key={w} className="attendance-calendar__weekday">
                  {w}
                </span>
              ))}
              {Array.from({ length: leadingBlanks }).map((_, i) => (
                <span key={`blank-${i}`} className="attendance-calendar__day is-outside" />
              ))}
              {home.attendance.calendar.map((d) => {
                const todayStr = new Date().toISOString().slice(0, 10);
                const isToday = d.date === todayStr;
                const isFuture = d.date > todayStr;
                const cls = [
                  "attendance-calendar__day",
                  d.done ? "is-done" : isFuture ? "is-future" : "is-missed",
                  isToday ? "is-today" : "",
                ].join(" ");
                return (
                  <span
                    key={d.date}
                    className={cls}
                    title={`${d.date} · ${d.done ? `${t.signed} +${d.reward}` : isFuture ? "" : t.unsigned}`}
                  >
                    <strong>{d.day}</strong>
                    <small>{d.done ? `+${d.reward}` : isFuture ? "\u00A0" : t.unsignedShort}</small>
                  </span>
                );
              })}
            </div>
            <footer className="attendance-card__legend">
              <span>
                <i className="is-done" /> {t.legendDone}
              </span>
              <span>
                <i className="is-today" /> {t.legendToday}
              </span>
            </footer>
          </aside>
        );
      case "shoutbox":
        return <ShoutBox />;
      case "funbox":
        return <FunBox embedded />;
      case "resource_stats":
        return <ResourceStatsPanel series={home.resource_stats} t={t} />;
      case "site_data":
        return (
          <section className="baozi-panel home-site-data">
            <header className="baozi-panel__head">
              <h2>
                <span aria-hidden="true">▦</span> {t.siteDataTitle}
              </h2>
              <small>{t.siteDataNote}</small>
            </header>
            <div className="home-site-data__grid">
              <dl className="home-site-data__column">
                <div className="home-site-data__item is-primary">
                  <dt>{t.sdTodayUsers}</dt>
                  <dd className="num">{home.site_data.peers.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{dict.home.torrents}</dt>
                  <dd className="num">{home.site_data.torrents.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{t.sdPeers}</dt>
                  <dd className="num">{home.site_data.peers.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{t.sdSeeders}</dt>
                  <dd className="num">{home.site_data.seeders.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{t.sdLeechers}</dt>
                  <dd className="num">{home.site_data.leechers.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{t.sdTotalDown}</dt>
                  <dd className="num">{fmtBytes(home.site_data.total_download)}</dd>
                </div>
              </dl>
              <dl className="home-site-data__column">
                <div className="home-site-data__item is-primary">
                  <dt>{t.sdUsers}</dt>
                  <dd className="num">{home.site_data.users.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item is-warning">
                  <dt>
                    {t.sdWarned}
                    <i aria-hidden="true">!</i>
                  </dt>
                  <dd className="num">{home.site_data.warned.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item is-danger">
                  <dt>
                    {t.sdBanned}
                    <i aria-hidden="true">×</i>
                  </dt>
                  <dd className="num">{home.site_data.banned.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{t.sdSeedLeechRatio}</dt>
                  <dd className="num">
                    {home.site_data.leechers === 0
                      ? "∞"
                      : `${((home.site_data.seeders / Math.max(1, home.site_data.leechers)) * 100).toFixed(0)}%`}
                  </dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{dict.home.seedSize}</dt>
                  <dd className="num">{fmtBytes(home.site_data.total_size)}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{t.sdTotalData}</dt>
                  <dd className="num">
                    {fmtBytes(home.site_data.total_upload + home.site_data.total_download)}
                  </dd>
                </div>
              </dl>
              <dl className="home-site-data__column">
                <div className="home-site-data__item is-primary">
                  <dt>{t.sdUnverified}</dt>
                  <dd className="num">{home.site_data.unverified.toLocaleString()}</dd>
                </div>
                <div className="home-site-data__item">
                  <dt>{t.sdTotalUp}</dt>
                  <dd className="num">{fmtBytes(home.site_data.total_upload)}</dd>
                </div>
              </dl>
            </div>
          </section>
        );
      case "lucky_draw":
        return (
          <aside className="baozi-panel home-lucky-draw" aria-label={t.luckyTitle}>
            <header className="baozi-panel__head">
              <h2>
                <span aria-hidden="true">🎰</span> {t.luckyTitle}
              </h2>
              <Link href="/games">{t.goDraw}</Link>
            </header>
            <ul className="home-lucky-draw__list">
              {home.lucky_draw.map((l, i) => (
                <li key={i}>
                  <b className="rainbow">{l.user}</b> {t.got} {dict.common.spark.replace("{magic}", currency)} {l.amount}
                </li>
              ))}
              {home.lucky_draw.length === 0 && <li className="text-sub">{t.luckyEmpty}</li>}
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
                <a key={l.url} href={l.url} title={l.title ?? l.name} target="_blank" rel="noreferrer">
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
          <div key={item.key} className={`home-cell home-cell--span-${effectiveSpan(item)}`}>
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

function ResourceStatsPanel({
  series,
  t,
}: {
  series: HomeData["resource_stats"];
  t: NonNullable<ReturnType<typeof useI18n>["dict"]>["home2"];
}) {
  const max = Math.max(1, ...series.series.map((d) => d.total));
  return (
    <section className="baozi-panel home-resource-stats">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">▥</span> {t.statsTitle}
        </h2>
        <small>{t.statsNote}</small>
      </header>
      <div className="home-resource-stats__metrics">
        <div className="home-resource-stats__metric">
          <span>{t.statsToday}</span>
          <strong className="num">{series.today}</strong>
          <small>{t.statsAsOfNow}</small>
        </div>
        <div className="home-resource-stats__metric">
          <span>{t.statsAvg7}</span>
          <strong className="num">{series.avg7.toFixed(1)}</strong>
          <small>{t.statsExclToday}</small>
        </div>
        <div className="home-resource-stats__metric">
          <span>{t.statsTotal30}</span>
          <strong className="num">{series.total30.toLocaleString()}</strong>
          <small>{t.statsResource}</small>
        </div>
        <div className="home-resource-stats__legend">
          <span>
            <i className="legend-swatch legend-swatch--ordinary" aria-hidden="true" />{" "}
            {t.statsOrdinary}
          </span>
          <span>
            <i className="legend-swatch legend-swatch--official" aria-hidden="true" /> {t.statsOfficial}
          </span>
        </div>
      </div>
      <figure className="home-resource-stats__figure">
        <div className="home-resource-stats__chart" role="img" aria-label={t.statsChartAria}>
          {series.series.map((d) => (
            <div key={d.date} className="rs-bar" title={`${d.date}：${t.statsOrdinary} ${d.ordinary}，${t.statsOfficial} ${d.official}，${t.statsTotalShort} ${d.total}`}>
              {d.official > 0 && (
                <i className="rs-bar__official" style={{ flexGrow: d.official / max }} />
              )}
              <i className="rs-bar__ordinary" style={{ flexGrow: d.ordinary / max }} />
            </div>
          ))}
        </div>
      </figure>
    </section>
  );
}
