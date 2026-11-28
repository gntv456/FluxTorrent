import { getPreserve } from "@/lib/data";
import { ErrorPanel } from "@/components/ui/error-panel";
import { normalizeError } from "@/lib/render-error";
import Link from "next/link";
import { getDict } from "@/i18n/server";
import {
  getSiteProfile,
  getTorrentDicts,
  catPath,
  colorMap,
} from "@/lib/site-profile";
import { fmt } from "@/i18n/config";
import { TorrentTr } from "@/components/torrent-table";
import {
  TorrentCards,
  TorrentPosters,
} from "@/app/(main)/torrents/_parts/torrents-cards";
import { parseView } from "@/app/(main)/torrents/_parts/torrents-utils";
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
  // 四路并发（2026-11-27 首屏提速：此前 requireModule→getDict→
  // getTorrentDicts→getPreserve 四连串行，每步一次 API RTT，线上跨
  // CF 串出 2-4s 白屏；四者互相无依赖，同 layout.tsx 布局并发先例）。
  // searchParams 先行解包（同步拿筛选参数，getPreserve 的入参立刻就绪）。
  const spRaw = await searchParams;
  const pick = (k: string): string | undefined => {
    const v = spRaw[k];
    const s = Array.isArray(v) ? v[0] : v;
    return s && s !== "all" && s !== "current" ? s : undefined;
  };
  /** 视图切换链接（RSC 链接式）：保留现有筛选参数，替换 view 参数 */
  const withView = (v: string): string => {
    const qs = new URLSearchParams();
    for (const [k, val] of Object.entries(spRaw)) {
      const s = Array.isArray(val) ? val[0] : val;
      if (s && k !== "view") qs.set(k, s);
    }
    qs.set("view", v);
    const s = qs.toString();
    return s ? `/preserve?${s}` : "/preserve";
  };
  // P-286：取数失败必须显式报错，不能演成「暂无保种需求」。
  let data: Awaited<ReturnType<typeof getPreserve>>;
  const [gate, dictPack, torrentDicts, profile, dataOrErr] = await Promise.all([
    requireModule("preserve"),
    getDict(),
    getTorrentDicts(),
    getSiteProfile().catch(() => null),
    getPreserve({
      scope: pick("scope"),
      status: pick("status"),
      category: pick("category"),
      keyword: pick("keyword"),
    }).then(
      (v) => v,
      (e: unknown) => e,
    ),
  ]);
  if (gate) return gate;
  const { dict } = dictPack;
  // 分类筛选项取自站点分类表（torrents.category_id 的外键目标）
  const cats = torrentDicts.categories;
  // 种子粒度视图（2026-11-28 站长口径：保种区与资源库完全同款——
  // table/card/poster 三视图切换，view 走 URL 参数与资源库共享口径）
  const view = parseView(
    Array.isArray(spRaw.view) ? spRaw.view[0] : spRaw.view,
  );
  // 分类图标键/分类色（card/poster 视图的封面色块）
  const catIcons = Object.fromEntries(
    (profile?.categories ?? [])
      .filter((c) => c.icon_key)
      .map((c) => [c.id, c.icon_key]),
  ) as Record<number, string>;
  const catColors = colorMap(profile?.categories ?? []);
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
  const isErr =
    dataOrErr instanceof Error ||
    !(dataOrErr as { items?: unknown } | null)?.items;
  if (isErr) {
    return (
      <ErrorPanel
        error={normalizeError(dataOrErr)}
        retryHref="/preserve"
      />
    );
  }
  data = dataOrErr as Awaited<ReturnType<typeof getPreserve>>;
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
          {/* 2026-11-27 新口径明示：入区/免费/出区规则一行讲清 */}
          <small className="preserve-hero__rule">{dict.preserve.rule}</small>
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
          {/* 视图切换（2026-11-28 站长口径：与资源库完全同款）：URL 链接式
              三态（table/card/poster），RSC 直出无 client 开销 */}
          <span className="ml-auto flex gap-1">
            {(
              [
                ["table", dict.torrents.viewTable],
                ["card", dict.torrents.viewCard],
                ["poster", dict.torrents.viewPoster],
              ] as const
            ).map(([v, label]) => (
              <Link
                key={v}
                href={withView(v)}
                className={`rounded px-2 py-1 text-xs ${
                  view === v
                    ? "bg-brand-soft font-bold text-brand"
                    : "text-sub hover:text-brand"
                }`}
              >
                {label}
              </Link>
            ))}
          </span>
        </div>
        {items.length === 0 ? (
          <p className="py-10 text-center text-sub">{dict.preserve.empty}</p>
        ) : view === "card" ? (
          <TorrentCards
            items={items as never}
            dict={dict}
            colors={catColors}
          />
        ) : view === "poster" ? (
          <TorrentPosters
            items={items as never}
            dict={dict}
            colors={catColors}
          />
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
            {dict.preserve.rssNote}{" "}
            {/* 2026-11-27 保种区订阅直达：preserve=1 → RSS 只回在区种子
                （getrss 构建器读 searchParams 预填「仅保种区」勾选） */}
            <Link
              href="/getrss?preserve=1"
              className="text-link hover:underline"
            >
              📡 {dict.preserve.rssTitle}
            </Link>
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
