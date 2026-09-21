// 首页站点数据板块（从 home-sections.tsx 按域拆出，300 门禁）：
// 用户/种子/做种三列统计卡。数据契约与 fmtBytes 在 home-data.ts。
import {
  fmtBytes,
  type DictT,
  type Home2T,
  type HomeData,
} from "@/components/home-data";

export function SiteDataCard({
  home,
  t,
  dict,
}: {
  home: HomeData;
  t: Home2T;
  dict: DictT;
}) {
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
            <dd className="num">{home.site_data.users.toLocaleString()}</dd>
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
              {fmtBytes(
                home.site_data.total_upload + home.site_data.total_download,
              )}
            </dd>
          </div>
        </dl>
        <dl className="home-site-data__column">
          <div className="home-site-data__item is-primary">
            <dt>{t.sdUnverified}</dt>
            <dd className="num">
              {home.site_data.unverified.toLocaleString()}
            </dd>
          </div>
          <div className="home-site-data__item">
            <dt>{t.sdTotalUp}</dt>
            <dd className="num">{fmtBytes(home.site_data.total_upload)}</dd>
          </div>
        </dl>
      </div>
    </section>
  );
}
