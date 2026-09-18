import { getPreserve } from "@/lib/data";
import Link from "next/link";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { ClaimButton } from "@/components/claim-button";
import { TorrentTr } from "@/components/torrent-table";
import { SeedStatsCard } from "@/components/seed-stats-card";
import { ResurrectionPanel } from "@/components/resurrection-panel";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 保种区（参考站 requireseed.php 复刻）：
 *  KEEP SHARING hero + 统计六格 + 筛选卡（类型/状态/分类/关键词）+ RSS 卡 + 种子表格 */
export default async function PreservePage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("preserve");
  if (gate) return gate;

  const { dict } = await getDict();
  // 筛选接线（审计修复 P1）：scope/status 表单参数此前后端声明即弃、前端也不透传——
  // 筛选 UI 双重失效。现在透传到 /preserve。
  const spRaw = await searchParams;
  const pick = (k: string): string | undefined => {
    const v = spRaw[k];
    const s = Array.isArray(v) ? v[0] : v;
    return s && s !== "all" && s !== "current" ? s : undefined;
  };
  const { items, stats, total } = await getPreserve({
    scope: pick("scope"),
    status: pick("status"),
    category: pick("category"),
    keyword: pick("keyword"),
  });
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

      {/* 保种统计：持 seed.stats.view 权限者可见（保种员 / 贵宾 / 管理组），无权限自动降级为提示 */}
      <SeedStatsCard />

      {/* 复活任务（0073）：可领取死种列表 + 我的任务 */}
      <ResurrectionPanel />

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
          <table className="nexus-table torrents-table preserve-list-table">
            <thead>
              <tr>
                <th className="w-12">{dict.torrents.colType}</th>
                <th className="w-16" aria-label="封面" />
                <th>{dict.torrents.colTitle}</th>
                <th className="w-16" title={dict.torrents.colComments}>
                  💬
                </th>
                <th className="w-20" title={dict.torrents.alive}>
                  ⏱
                </th>
                <th className="w-20" title={dict.torrents.colSize}>
                  💾
                </th>
                <th className="w-16" title={dict.torrents.colSeeders}>
                  🌱
                </th>
                <th className="w-16" title={dict.torrents.colLeechers}>
                  ⬇️
                </th>
                <th className="w-16" title={dict.torrents.colCompleted}>
                  ✅
                </th>
                {/* 保种区专属列：认领人 + 认领操作 */}
                <th className="w-24">{dict.preserve.claimedBy}</th>
                <th className="w-24">{dict.torrents.colActions}</th>
              </tr>
            </thead>
            <tbody>
              {items.map((p) => (
                <TorrentTr
                  key={p.torrent_id}
                  t={p}
                  extra={
                    <>
                      <td className="text-sub">
                        {p.claimed_by ?? dict.preserve.unclaimed}
                      </td>
                      <td className="text-right">
                        <ClaimButton
                          torrentId={p.torrent_id}
                          claimed={Boolean(p.claimed_by)}
                          label={dict.preserve.claimAction}
                          claimedLabel={dict.preserve.claimedTag}
                        />
                      </td>
                    </>
                  }
                />
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
