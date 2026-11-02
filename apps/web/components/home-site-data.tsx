// 首页站点数据板块（从 home-sections.tsx 按域拆出，300 门禁）：
//  用户/种子/做种统计卡。数据契约与 fmtBytes 在 home-data.ts。
//
//  两套排版（同一份数据、两个容器，靠 CSS 择一显示）：
//   桌面 —— 三列编排（与 2026-10 起的原样一致，只去掉重复的「注册用户」）；
//   手机 —— 两列网格 + 「更多统计」折叠（低位指标收进去）。
//  手机端原先是 15 行满宽单列（691px）：`.home-site-data__grid` 用的是
//  `minmax(230px,1fr)`，366px 的容器只够排 1 列，于是每个指标独占一行。
import {
  fmtBytes,
  type DictT,
  type Home2T,
  type HomeData,
} from "@/components/home-data";

/** 一个指标格：标签 + 数值；tone 决定强调色（与原 is-primary/warning/danger 同义）。 */
interface SdItem {
  key: string;
  label: string;
  value: string;
  tone?: "primary" | "warning" | "danger";
}

function SdRow({ item }: { item: SdItem }) {
  return (
    <div
      className={
        "home-site-data__item" + (item.tone ? ` is-${item.tone}` : "")
      }
    >
      <dt>
        {item.label}
        {item.tone === "warning" ? <i aria-hidden="true">!</i> : null}
        {item.tone === "danger" ? <i aria-hidden="true">×</i> : null}
      </dt>
      <dd className="num">{item.value}</dd>
    </div>
  );
}

export function SiteDataCard({
  home,
  t,
  dict,
}: {
  home: HomeData;
  t: Home2T;
  dict: DictT;
}) {
  const sd = home.site_data;
  const n = (v: number) => v.toLocaleString();
  // 无下载时显示「—」而非 ∞（评审 P2-12）：空站 ∞ 是除零假象
  const ratioText =
    sd.leechers === 0
      ? "—"
      : `${((sd.seeders / Math.max(1, sd.leechers)) * 100).toFixed(0)}%`;

  // 主指标（手机两列网格直接铺这 8 个）与低位指标（收进「更多统计」）
  const main: SdItem[] = [
    { key: "users", label: t.sdUsers, value: n(sd.users), tone: "primary" },
    { key: "torrents", label: dict.home.torrents, value: n(sd.torrents) },
    { key: "peers", label: t.sdPeers, value: n(sd.peers) },
    { key: "seeders", label: t.sdSeeders, value: n(sd.seeders) },
    { key: "leechers", label: t.sdLeechers, value: n(sd.leechers) },
    {
      key: "seed_size",
      label: dict.home.seedSize,
      value: fmtBytes(sd.total_size),
    },
    {
      key: "total_data",
      label: t.sdTotalData,
      value: fmtBytes(sd.total_upload + sd.total_download),
    },
    {
      key: "total_up",
      label: t.sdTotalUp,
      value: fmtBytes(sd.total_upload),
    },
  ];
  const extra: SdItem[] = [
    {
      key: "total_down",
      label: t.sdTotalDown,
      value: fmtBytes(sd.total_download),
    },
    {
      key: "warned",
      label: t.sdWarned,
      value: n(sd.warned),
      tone: "warning",
    },
    {
      key: "banned",
      label: t.sdBanned,
      value: n(sd.banned),
      tone: "danger",
    },
    { key: "unverified", label: t.sdUnverified, value: n(sd.unverified) },
    {
      key: "seed_leech_ratio",
      label: t.sdSeedLeechRatio,
      value: ratioText,
    },
  ];
  const byKey = (k: string) => [...main, ...extra].find((x) => x.key === k)!;

  // 桌面三列编排（保持原视觉：资源/同伴/做种在前，警示项在中）
  const desktopColumns: SdItem[][] = [
    [
      byKey("users"),
      byKey("torrents"),
      byKey("peers"),
      byKey("seeders"),
      byKey("leechers"),
      byKey("total_down"),
    ],
    [
      byKey("warned"),
      byKey("banned"),
      byKey("seed_leech_ratio"),
      byKey("seed_size"),
      byKey("total_data"),
    ],
    [byKey("unverified"), byKey("total_up")],
  ];

  return (
    <section className="brand-panel home-site-data">
      <header className="brand-panel__head">
        <h2>
          <span aria-hidden="true">▦</span> {t.siteDataTitle}
        </h2>
        <small>{t.siteDataNote}</small>
      </header>
      {/* 桌面三列 */}
      <div className="home-site-data__grid">
        {desktopColumns.map((col, i) => (
          <dl className="home-site-data__column" key={i}>
            {col.map((it) => (
              <SdRow item={it} key={it.key} />
            ))}
          </dl>
        ))}
      </div>
      {/* 手机两列网格 */}
      <dl className="home-site-data__grid home-site-data__grid--m">
        {main.map((it) => (
          <SdRow item={it} key={it.key} />
        ))}
      </dl>
      {/* 手机：低位指标折叠 */}
      <details className="home-site-data__more">
        <summary>{t.sdMore}</summary>
        <dl>
          {extra.map((it) => (
            <SdRow item={it} key={it.key} />
          ))}
        </dl>
      </details>
    </section>
  );
}
