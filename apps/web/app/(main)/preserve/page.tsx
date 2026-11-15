import { getPreserve } from "@/lib/data";
import { ErrorPanel } from "@/components/ui/error-panel";
import { normalizeError } from "@/lib/render-error";
import Link from "next/link";
import { getDict } from "@/i18n/server";
import { getTorrentDicts, catPath } from "@/lib/site-profile";
import { fmt } from "@/i18n/config";
import { ClaimButton } from "@/components/claim-button";
import { TorrentTr } from "@/components/torrent-table";
import { SeedStatsCard } from "@/components/seed-stats-card";
import { requireModule } from "@/components/module-gate";
import { Icon } from "@/components/icons";

export const dynamic = "force-dynamic";

/** 保种区（参考站 requireseed.php 复刻；2026-11-15 版式重构）：
 *  Hero（内嵌统计一行）→ 保种资源列表（主位）→ 工具抽屉（筛选+RSS+复活引导）
 *  → 保种统计卡（权限折叠）。此前六块全宽平铺把资源表压到第 4 屏，
 *  会员进来先看一堆装饰，翻三次才见到「要保什么」。 */
export default async function PreservePage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("preserve");
  if (gate) return gate;

  const { dict } = await getDict();
  // 分类筛选项取自站点分类表（torrents.category_id 的外键目标）
  const cats = (await getTorrentDicts()).categories;
  // 筛选接线（审计修复 P1）：scope/status 表单参数此前后端声明即弃、前端也不透传——
  // 筛选 UI 双重失效。现在透传到 /preserve。
  const spRaw = await searchParams;
  const pick = (k: string): string | undefined => {
    const v = spRaw[k];
    const s = Array.isArray(v) ? v[0] : v;
    return s && s !== "all" && s !== "current" ? s : undefined;
  };
  // 受控回填：把当前筛选值回显到表单控件（避免刷新后选择丢失）。
  // 有任一筛选生效时自动展开工具抽屉（高级筛选 advAutoOpen 同理：
  // 用户带着条件进来，抽屉不该再藏起来）。
  const rawVal = (k: string, dflt: string): string => {
    const v = spRaw[k];
    const s = Array.isArray(v) ? v[0] : v;
    return s ?? dflt;
  };
  const filtered = Boolean(
    pick("scope") ?? pick("status") ?? pick("category") ?? pick("keyword"),
  );
  // P-286：取数失败必须显式报错，不能演成「暂无保种需求」
  let data: Awaited<ReturnType<typeof getPreserve>>;
  try {
    data = await getPreserve({
      scope: pick("scope"),
      status: pick("status"),
      category: pick("category"),
      keyword: pick("keyword"),
    });
  } catch (e) {
    return <ErrorPanel error={normalizeError(e)} retryHref="/preserve" />;
  }
  const { items, stats, total } = data;
  const statCells: [string, number][] = [
    [dict.preserve.stPreserving, stats.preserving],
    [dict.preserve.stContinued, stats.continued],
    [dict.preserve.stOfficial, stats.official],
    [dict.preserve.stGeneral, stats.general],
    [dict.preserve.stTodayIn, stats.today_in],
    [dict.preserve.stTodayOut, stats.today_out],
  ];
  return (
    <div className="preserve-page">
      {/* Hero + 内嵌统计一行（原先统计六格独占一屏块，压缩进 Hero） */}
      <section className="brand-utility-hero preserve-hero">
        <span className="preserve-hero__mark" aria-hidden="true">
          🌱
        </span>
        <span className="preserve-hero__copy">
          <small>KEEP SHARING · KEEP GROWING</small>
          <strong>{dict.preserve.title}</strong>
          <span>{dict.preserve.subtitle}</span>
        </span>
        <span className="preserve-hero__stats">
          {statCells.map(([label, n]) => (
            <span key={label} className="preserve-hero__stat">
              <strong className="num">{n.toLocaleString()}</strong>
              <small>{label}</small>
            </span>
          ))}
        </span>
      </section>

      {/* 保种资源列表（主位：紧跟 Hero，核心内容首屏可见） */}
      <section className="brand-panel preserve-list-card">
        <div className="preserve-list-card__heading">
          <h2>{dict.preserve.listTitle}</h2>
          <span>
            {data.bonus_per_day
              ? `${dict.preserve.bonusPerDay.replace(
                  "{n}",
                  String(data.bonus_per_day),
                )} · `
              : ""}
            {fmt(dict.preserve.totalResults, { n: total.toLocaleString() })}
          </span>
        </div>
        {items.length === 0 ? (
          <p className="py-10 text-center text-sub">{dict.preserve.empty}</p>
        ) : (
          <div className="brand-wide-table-scroll">
            <table className="nexus-table torrents-table preserve-list-table">
              <thead>
                <tr>
                  <th className="w-12">{dict.torrents.colType}</th>
                  <th
                    className="w-16"
                    aria-label={dict.torrents.colNames.cover}
                  />
                  <th>{dict.torrents.colTitle}</th>
                  <th className="w-16" title={dict.torrents.colComments}>
                    <Icon
                      name="messages"
                      size={15}
                      className="inline align-[-3px]"
                    />
                  </th>
                  <th className="w-20" title={dict.torrents.alive}>
                    <Icon
                      name="clock"
                      size={15}
                      className="inline align-[-3px]"
                    />
                  </th>
                  <th className="w-20" title={dict.torrents.colSize}>
                    <Icon
                      name="disc"
                      size={15}
                      className="inline align-[-3px]"
                    />
                  </th>
                  <th className="w-16" title={dict.torrents.colSeeders}>
                    <Icon
                      name="users"
                      size={15}
                      className="inline align-[-3px]"
                    />
                  </th>
                  <th className="w-16" title={dict.torrents.colLeechers}>
                    <Icon
                      name="download"
                      size={15}
                      className="inline align-[-3px]"
                    />
                  </th>
                  <th className="w-16" title={dict.torrents.colCompleted}>
                    <Icon
                      name="check"
                      size={15}
                      className="inline align-[-3px]"
                    />
                  </th>
                  {/* 保种区专属列：认领人 + 认领操作 */}
                  <th className="w-24">{dict.preserve.claimedByCol}</th>
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
                            bonus={data.bonus_per_day}
                          />
                        </td>
                      </>
                    }
                  />
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* 工具抽屉：筛选 + RSS + 复活引导收进一张 details（默认收起，
          带筛选参数进来时自动展开；RSS 卡只剩一句提示，一并收纳） */}
      <details
        className="brand-panel preserve-tools"
        open={filtered || undefined}
      >
        <summary>{dict.preserve.filterTitle}</summary>
        <div className="preserve-tools__body">
          <form className="preserve-filter" action="/preserve" method="get">
            <label>
              <span>{dict.preserve.scope}</span>
              <select name="scope" defaultValue={rawVal("scope", "all")}>
                <option value="all">{dict.preserve.scopeAll}</option>
                <option value="official">{dict.preserve.stOfficial}</option>
                <option value="general">{dict.preserve.stGeneral}</option>
              </select>
            </label>
            <label>
              <span>{dict.preserve.status}</span>
              <select name="status" defaultValue={rawVal("status", "current")}>
                <option value="current">
                  {dict.preserve.statusCurrent}
                </option>
                <option value="active">{dict.preserve.stPreserving}</option>
                <option value="grace">{dict.preserve.stContinued}</option>
                <option value="expired">{dict.preserve.statusExpired}</option>
              </select>
            </label>
            <label>
              <span>{dict.preserve.category}</span>
              <select name="category" defaultValue={rawVal("category", "0")}>
                <option value="0">{dict.preserve.categoryAll}</option>
                {cats.map((c) => (
                  <option key={c.id} value={c.id}>
                    {catPath(cats, c)}
                  </option>
                ))}
              </select>
            </label>
            <label className="preserve-filter__keyword">
              <span>{dict.preserve.keyword}</span>
              <input
                type="search"
                name="keyword"
                defaultValue={rawVal("keyword", "")}
                maxLength={100}
                placeholder={dict.preserve.keywordPh}
                autoComplete="off"
              />
            </label>
            <div className="preserve-filter__actions">
              <button className="brand-button" type="submit">
                {dict.requests.search}
              </button>
              <a className="preserve-filter__reset" href="/preserve">
                {dict.preserve.reset}
              </a>
            </div>
          </form>
          <p className="preserve-tools__rss">
            <strong>{dict.preserve.rssTitle}</strong>
            {dict.preserve.rssNote}
          </p>
          <p className="text-sm text-muted">
            <Link href="/resurrections" className="text-link hover:underline">
              {dict.preserve.resurrectionHint}
            </Link>
          </p>
        </div>
      </details>

      {/* 保种统计：持 seed.stats.view 权限者可见（保种员 / 贵宾 / 管理组），无权限自动降级为提示 */}
      <SeedStatsCard />
    </div>
  );
}
