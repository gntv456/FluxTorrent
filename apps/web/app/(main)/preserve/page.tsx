import { getPreserve } from "@/lib/data";
import { formatBytes } from "@/lib/format";
import Link from "next/link";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { ClaimButton } from "@/components/claim-button";

export const dynamic = "force-dynamic";

/** 保种区（包子站 requireseed.php 复刻）：
 *  KEEP SHARING hero + 统计六格 + 筛选卡（类型/状态/分类/关键词）+ RSS 卡 + 种子表格 */
export default async function PreservePage() {
  const { dict } = await getDict();
  const { items, stats, total } = await getPreserve();
  return (
    <div className="preserve-page">
      {/* Hero */}
      <section className="baozi-utility-hero preserve-hero">
        <span className="preserve-hero__mark" aria-hidden="true">
          🌱
        </span>
        <span className="preserve-hero__copy">
          <small>KEEP SHARING · KEEP GROWING</small>
          <strong>{dict.preserve.title}</strong>
          <span>{dict.preserve.subtitle}</span>
        </span>
      </section>

      {/* 统计六格 */}
      <section className="preserve-stats">
        <div>
          <strong className="num">{stats.preserving.toLocaleString()}</strong>
          <span>{dict.preserve.stPreserving}</span>
        </div>
        <div>
          <strong className="num">{stats.continued.toLocaleString()}</strong>
          <span>{dict.preserve.stContinued}</span>
        </div>
        <div>
          <strong className="num">{stats.official.toLocaleString()}</strong>
          <span>{dict.preserve.stOfficial}</span>
        </div>
        <div>
          <strong className="num">{stats.general.toLocaleString()}</strong>
          <span>{dict.preserve.stGeneral}</span>
        </div>
        <div>
          <strong className="num">{stats.today_in.toLocaleString()}</strong>
          <span>{dict.preserve.stTodayIn}</span>
        </div>
        <div>
          <strong className="num">{stats.today_out.toLocaleString()}</strong>
          <span>{dict.preserve.stTodayOut}</span>
        </div>
      </section>

      {/* 筛选卡 */}
      <section className="baozi-panel preserve-filter-card">
        <h2>{dict.preserve.filterTitle}</h2>
        <form className="preserve-filter" action="/preserve" method="get">
          <label>
            <span>{dict.preserve.scope}</span>
            <select name="scope">
              <option value="all">{dict.preserve.scopeAll}</option>
              <option value="official">{dict.preserve.stOfficial}</option>
              <option value="general">{dict.preserve.stGeneral}</option>
            </select>
          </label>
          <label>
            <span>{dict.preserve.status}</span>
            <select name="status">
              <option value="current">{dict.preserve.statusCurrent}</option>
              <option value="active">{dict.preserve.stPreserving}</option>
              <option value="grace">{dict.preserve.stContinued}</option>
              <option value="expired">{dict.preserve.statusExpired}</option>
            </select>
          </label>
          <label>
            <span>{dict.preserve.category}</span>
            <select name="category">
              <option value="0">{dict.preserve.categoryAll}</option>
              {dict.torrents.categories
                .filter((c) => c !== dict.torrents.categories[0])
                .map((c, i) => (
                  <option key={c} value={i + 1}>
                    {c}
                  </option>
                ))}
            </select>
          </label>
          <label className="preserve-filter__keyword">
            <span>{dict.preserve.keyword}</span>
            <input
              type="search"
              name="keyword"
              maxLength={100}
              placeholder={dict.preserve.keywordPh}
              autoComplete="off"
            />
          </label>
          <div className="preserve-filter__actions">
            <button className="baozi-button" type="submit">
              {dict.requests.search}
            </button>
            <a className="preserve-filter__reset" href="/preserve">
              {dict.preserve.reset}
            </a>
          </div>
        </form>
      </section>

      {/* RSS 订阅卡 */}
      <section className="baozi-panel preserve-rss">
        <h2>{dict.preserve.rssTitle}</h2>
        <p>{dict.preserve.rssNote}</p>
      </section>

      {/* 保种资源列表 */}
      <section className="baozi-panel preserve-list-card">
        <div className="preserve-list-card__heading">
          <h2>{dict.preserve.listTitle}</h2>
          <span>
            {fmt(dict.preserve.totalResults, { n: total.toLocaleString() })}
          </span>
        </div>
        {items.length === 0 ? (
          <p className="py-10 text-center text-sub">{dict.preserve.empty}</p>
        ) : (
          <table className="nexus-table preserve-list-table">
            <tbody>
              <tr>
                <td className="colhead">{dict.torrents.colTitle}</td>
                <td className="colhead">{dict.preserve.claimedBy}</td>
                <td className="colhead text-right">{dict.torrents.colSize}</td>
                <td className="colhead text-right">{dict.torrents.colSeeders}</td>
                <td className="colhead text-right">{dict.preserve.action}</td>
              </tr>
              {items.map((p) => (
                <tr key={p.torrent_id}>
                  <td>
                    <Link href={`/torrent/${p.torrent_id}`} className="font-bold">
                      {p.name}
                    </Link>
                  </td>
                  <td className="text-sub">
                    {p.claimed_by ?? dict.preserve.unclaimed}
                  </td>
                  <td className="num text-right">{formatBytes(p.size)}</td>
                  <td className="num text-right text-mint">{p.seeders}</td>
                  <td className="text-right">
                    <ClaimButton
                      torrentId={p.torrent_id}
                      claimed={Boolean(p.claimed_by)}
                      label={dict.preserve.claimAction}
                      claimedLabel={dict.preserve.claimedTag}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
