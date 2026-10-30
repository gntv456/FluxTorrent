import "@/app/styles/works.css";
import { EmptyTorrents } from "@/components/torrent";
import { ErrorPanel } from "@/components/ui/error-panel";
import { normalizeError } from "@/lib/render-error";
import { api, paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { colorMap, catPath } from "@/lib/site-profile";
import { fmt } from "@/i18n/config";
import type {
  TorrentListItem,
  TorrentPage,
} from "@fluxtorrent/domain-types";
import { TorrentsSearchBox } from "./_parts/torrents-search-box";
import { MobileSearch } from "./_parts/torrents-mobile-search";
import { TorrentsPullRefresh } from "./_parts/refresh-mount";
import { TorrentSplitView } from "./_parts/torrent-split";
import { TorrentsViewSwitch } from "./_parts/torrents-view-switch";
import { TorrentsColSettings } from "./_parts/torrents-col-settings";
import { TorrentListViews } from "./_parts/torrents-list-views";
import { TorrentWorks } from "./_parts/torrents-works";
import { TorrentsHotkeys } from "./_parts/torrents-hotkeys";
import { TorrentHoverPreview } from "@/components/torrent-hover-preview";
import { buildTorrentChips } from "./_parts/torrents-chips";
import {
  loadPublic,
  parsePageSize,
  parseView,
  toggleSort,
  withParam, withoutCursor,
  type SectionDictRow,
  type SectionKindMeta,
} from "./_parts/torrents-utils";

export const dynamic = "force-dynamic";

/** 翻页按钮样式（0170 双向翻页；长 className 拆串守行宽门禁） */
const PILL_CLS =
  "flex min-h-[44px] items-center rounded-full" +
  " border border-line bg-[var(--surface-card)] px-6 text-sm text-sky-deep";

/** 聚合视图不消费种子列表游标；不为此白跑一次 /torrents 查询。 */
const EMPTY_PAGE: TorrentPage<TorrentListItem> = {
  items: [], next_cursor: null, total_estimate: 0, hidden_dead: 0,
};

/** 种子页：常驻搜索行 + 折叠高级面板 + 已选条件摘要；拆件见 ./_parts/。 */
/** 「看全部（含断种）」的链接：保留其余筛选，只把 alive 切到 0。
 *  与筛选变更一样必须丢旧游标——旧游标绑定旧结果集。 */
function hiddenDeadHref(
  sp: Record<string, string | undefined>,
): string {
  return withParam(withoutCursor(sp), "alive", "0");
}

export default async function TorrentsPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const spRaw = await searchParams;
  // 归一化：重复参数（多选分类 checkbox）→ 逗号串；单值原样
  const sp: Record<string, string | undefined> = Object.fromEntries(
    Object.entries(spRaw).map(([k, v]) => [
      k,
      Array.isArray(v) ? v.join(",") : v,
    ]),
  );
  // 旧/外部链接归一：group→聚合+卡片；media→聚合+海报墙；torrents→种子列表
  if (sp.view === "group") { sp.view = "card"; sp.group = "1"; }
  if (sp.view === "media") { sp.view = "poster"; sp.group = "1"; }
  if (sp.view === "torrents") { sp.view = "table"; sp.group = undefined; }
  const grouped = sp.group === "1";
  if (grouped) sp.approval = undefined; // 作品层只展示过审资源
  const { dict, locale } = await getDict();
  // 视图/条数走 URL，未指定时回落用户偏好（0316）再回落默认；
  // 与字典链并行取（2026-10-18 提速：此前串行多等一个 API RTT）。
  // 多维筛选参数转发（sec_{kind} → 后端通用解析）：不转发则筛选静默失效
  const secParams: Record<string, string> = {};
  for (const [k, v] of Object.entries(sp)) {
    if (k.startsWith("sec_") && v) secParams[k] = v;
  }
  // 双链并发（2026-10-18）：字典与「偏好→列表」并行，尾延不再是两段之和。
  const dictsPromise = (async () => {
    const [profile, secDict, tagDict] = await Promise.all([
    loadPublic<{
      categories: {
        id: number;
        name: string;
        icon_key?: string;
        bg_color?: string | null;
      }[];
      metadata_sources?: string[];
      view_hidden?: { columns?: string[]; sections?: string[] };
    }>("/api/v1/site-profile"),
    loadPublic<
      Record<string, SectionDictRow[]> & { kinds?: SectionKindMeta[] }
    >("/api/v1/section-dict"),
    loadPublic<
      | {
          tags:
            | { id: number; name: string; kind: string }[]
            | [number, string, string][];
          groups?: string[];
        }
      | { id: number; name: string; kind: string }[]
      | [number, string, string][]
    >("/api/v1/tags-dict"),
    ]);
    return { profile, secDict, tagDict };
  })();
  const listChain = (async () => {
    const userPrefs = await api
      .get<{
        view?: string;
        per_page?: number;
        hidden_cols?: string[];
      }>("/api/v1/me/list-prefs")
      .catch(() => null);
    const view = sp.view
      ? parseView(sp.view)
      : parseView(userPrefs?.view ?? "table");
    const pageSize = sp.limit
      ? parsePageSize(sp.limit)
      : parsePageSize(
          userPrefs?.per_page ? String(userPrefs.per_page) : "20",
        );
    let loadErr: unknown = null;
    const page = grouped
      ? EMPTY_PAGE
      : await paged<TorrentListItem, TorrentPage<TorrentListItem>>(
      "/api/v1/torrents",
      {
      limit: pageSize,
      // 多选分类（0088）：后端 category_ids 兼容逗号串
      category_id: sp.category_id,
      official: sp.official ? sp.official === "1" : undefined,
      search: sp.search,
      // 搜索范围/匹配模式（NP 口径 0=标题 1=简介 3=发布者 4=IMDb；mode 2=精确）：
      // 落在 URL 却不转发会让下拉选择静默失效
      search_area: sp.search_area || undefined,
      search_mode: sp.search_mode || undefined,
      // 0102 高级搜索三态 + 多维多选（secParams 已是逗号串，后端 ANY 解析）
      alive: sp.alive || undefined,
      status: sp.status || undefined,
      approval: sp.approval || undefined,
      sort: sp.sort,
      // 标签多选（0159 P1）：tag_ids 逗号串优先，旧单值 tag_id 兜底（后端合并解析）
      tag_ids: sp.tag_ids || sp.tag_id || undefined,
      tag_mode: sp.tag_mode || undefined,
      cursor: sp.cursor,
      // 0170 双向翻页：dir=prev 取游标之前的页（服务端反向 keyset）
      dir: sp.dir || undefined,
      // 0105 高级搜索增强（体积带单位串 / 日期 YYYY-MM-DD / 数值区间 / 优惠 / 发布者 / 仅我）
      size_min: sp.size_min || undefined,
      size_max: sp.size_max || undefined,
      date_from: sp.date_from || undefined,
      date_to: sp.date_to || undefined,
      min_seeders: sp.min_seeders || undefined,
      max_seeders: sp.max_seeders || undefined,
      exclude: sp.exclude || undefined,
      promo: sp.promo || undefined,
      owner: sp.owner || undefined,
      mine: sp.mine || undefined,
      bookmarked: sp.bookmarked || undefined,
      // 0118 补齐（下载数/完成数区间 / 匿名发布 / 排序视图）
      min_leechers: sp.min_leechers || undefined,
      max_leechers: sp.max_leechers || undefined,
      min_completed: sp.min_completed || undefined,
      max_completed: sp.max_completed || undefined,
      rating_min: sp.rating_min || undefined,
      anonymous: sp.anonymous || undefined,
      ...secParams,
    }).catch((e) => {
      loadErr = e;
      return {
        items: [] as TorrentListItem[],
        next_cursor: null,
        total_estimate: 0,
        hidden_dead: 0,
      };
    });
    return { userPrefs, view, pageSize, page, loadErr };
  })();
  const [dictsRes, listRes] = await Promise.all([dictsPromise, listChain]);
  const { profile, secDict, tagDict } = dictsRes;
  const { userPrefs, view, pageSize, page, loadErr } = listRes;
  // E6 视图布局：站点级列隐藏集合 + 用户级覆盖（0316，title/行为列恒显）
  const hiddenCols = new Set([
    ...(profile?.view_hidden?.columns ?? []),
    ...(userPrefs?.hidden_cols ?? []),
  ]);
  const kinds: SectionKindMeta[] = secDict?.kinds ?? [];
  const dimKinds = kinds.filter((k) => (secDict?.[k.kind]?.length ?? 0) > 0);
  // 0160 起返回 { tags, groups }；旧形态（裸数组）兼容
  const tagRows =
    tagDict && !Array.isArray(tagDict) ? tagDict.tags : (tagDict ?? []);
  const tags = (tagRows ?? []).map((r) =>
    Array.isArray(r) ? { id: r[0], name: r[1], kind: r[2] } : r,
  );
  // 平行组别数组（0160 P2）：筛选面板按 attribute/content 分两行渲染
  const tagGroups =
    tagDict && !Array.isArray(tagDict) ? (tagDict.groups ?? []) : [];
  // 半旧会话/后端抖动降级为空列表，但「失败」与「没有结果」要分开（P0-6）。
  // 分类只以 site-profile 为准（后台可改），不回落 i18n 的筛选用词表——
  // 否则选项 id 会指到不存在的分类上。
  const cats = profile?.categories ?? [];
  const categories = cats.map((c) => ({ id: c.id, label: catPath(cats, c) }));
  // 分类图标键（0166）：{category_id: icon_key}；站长后台可替换，空键回落首字
  const catIcons = Object.fromEntries(
    (profile?.categories ?? [])
      .filter((c) => c.icon_key)
      .map((c) => [c.id, c.icon_key]),
  ) as Record<number, string>;
  // 分类色（0183）：categories.bg_color 下发，前端不再持有硬编码色表
  const catColors = colorMap(profile?.categories ?? []);
  // 多选分类：URL 里同名参数（checkbox 多选），解析去重
  const selectedCats = new Set(
    (sp.category_id ?? "").split(",").filter(Boolean).map(Number),
  );

  // ===== 已选条件摘要（每个 chip 可单独移除；「排序」不算筛选条件） =====
  const chips = buildTorrentChips({
    sp,
    dict,
    categories,
    tags,
    secDict,
    dimKinds,
    withParam,
  });
  const advancedOpen = chips.length > 0;
  const nonSearchChips = chips.filter((c) => c.key !== "search").length;
  // IMDb 范围随站点元数据源显隐（与上传页条目输入同口径）
  const showImdb =
    !profile || (profile.metadata_sources ?? []).includes("imdb");
  return (
    <div className="flex flex-col gap-3">
      <h1 className={sp.official === "1" ? "text-lg font-bold" : "sr-only"}>
        {sp.official === "1" ? dict.nav.official : dict.torrents.title}
      </h1>
      {/* 搜索盒：常驻一行 + 折叠高级面板（0105 重排）；
          <md 整块收进右下角「搜索」弹层，避免与旧筛选 FAB 重复 */}
      <MobileSearch>
        <TorrentsSearchBox
          dict={dict}
          sp={sp}
          categories={categories}
          tags={tags}
          tagGroups={tagGroups}
          secDict={secDict}
          dimKinds={dimKinds}
          selectedCats={selectedCats}
          chips={chips}
          advancedOpen={advancedOpen}
          nonSearchChips={nonSearchChips}
          showImdb={showImdb}
          withParam={withParam}
        />
      </MobileSearch>

      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="num text-xs text-sub">
          {grouped
            ? null
            : fmt(dict.torrents.total, { n: page.total_estimate })}
        </span>
        {/* 0102 存活三态的可见性补齐：默认视图只给活种，不说清楚的话
            新会员会把「列表只有 N 条」当成「本站只有 N 个种」。提示只在
            服务端真的藏了断种时出现，点了直接切到「全部」。RSC 里用链接
            不用 useRouter（本页是服务端组件）。 */}
        {(() => {
          // RSC 里用链接不用 useRouter（引它会拖成客户端组件）
          const n = page.hidden_dead ?? 0;
          // 作品聚合本就不含断种过滤，这个入口在聚合态不成立
          if (n <= 0 || grouped) return null;
          const href = hiddenDeadHref(sp);
          return (
            <a href={href} className="text-xs text-brand hover:underline">
              {fmt(dict.torrents.hiddenDead, { n })}
            </a>
          );
        })()}
        <div className="flex items-center gap-3">
          {view === "table" && !grouped && (
            <TorrentsColSettings
              initialHidden={userPrefs?.hidden_cols ?? []}
              siteHidden={profile?.view_hidden?.columns ?? []}
            />
          )}
          <TorrentsViewSwitch
            dict={dict}
            sp={sp}
            view={view}
            grouped={grouped}
            pageSize={pageSize}
            withParam={withParam}
          />
        </div>
      </div>
      {/* 键盘快捷键（/ 搜索、j/k 移动、Enter 打开）：挂载后短暂提示，8s 自动消失 */}
      <TorrentsHotkeys />
      {/* 列表 hover 预览卡（阶段三）：事件委托整页生效 */}
      <TorrentHoverPreview />

      {/* 编排轴 × 展示轴：聚合走作品粒度，不聚合走种子粒度 */}
      <TorrentSplitView items={page.items}>
      {grouped ? (
        // 作品聚合：按 view 分形态渲染，含断种；必须先于「列表为空」判断。
        <TorrentWorks
          dict={dict}
          locale={locale}
          sp={sp}
          cats={categories}
          view={view}
          perPage={pageSize}
          withParam={withParam}
        />
      ) : page.items.length === 0 ? (
        loadErr !== null ? (
          <ErrorPanel
            error={normalizeError(loadErr)}
            desc={dict.torrents.loadFailedHint}
            retryHref={withParam(sp, "cursor", undefined)}
          />
        ) : (
          <EmptyTorrents aliveFiltered={sp.alive !== "0" && sp.alive !== "2"} />
        )
      ) : (
        <TorrentListViews
          view={view}
          items={page.items}
          dict={dict}
          sp={sp}
          withParam={withParam}
          toggleSort={toggleSort}
          catIcons={catIcons}
          colors={catColors}
          hiddenCols={hiddenCols}
        />
      )}

      </TorrentSplitView>

      {/* M3：下拉刷新（仅触屏渲染，不影响桌面） */}
      <TorrentsPullRefresh />

      {/* RSS 订阅入口（NP 列表头 RSS 图标口径）：携带当前关键字/分类/维度
          跳转订阅页——维度筛选（B3）一并带走，否则「订阅当前结果」名不副实 */}
      <div className="flex justify-end">
        <a
          href={`/getrss?${new URLSearchParams({
            keyword: sp.search ?? "",
            categories: sp.category_id ?? "",
            ...secParams,
          }).toString()}`}
          className="text-xs text-sky hover:underline"
        >
          📡 {dict.getrss.title}
        </a>
      </div>

      {/* 双向翻页（0170）：正向页 = cursor + dir=prev 回上一页；反向页 =
          next_cursor 继续往回、删 dir 回正到游标原页。页码 p 仅作显示（服务端忽略） */}
      {!grouped && (() => {
        const pageNo = Math.max(1, Number(sp.p ?? "1") || 1);
        const isPrev = sp.dir === "prev";
        // 上一页：正向页带 dir=prev；反向页沿反向继续（next_cursor 为锚）
        const prevHref = isPrev
          ? page.next_cursor
            ? withParam(
                {
                  ...sp,
                  cursor: page.next_cursor,
                  p: String(Math.max(1, pageNo - 1)),
                },
                "dir",
                "prev",
              )
            : null
          : sp.cursor
            ? withParam(sp, "dir", "prev")
            : null;
        // 下一页：正向 = next_cursor 前进；反向 = 删 dir 回正（cursor 保持）
        const nextHref = isPrev
          ? withParam(sp, "dir", undefined)
          : page.next_cursor
            ? withParam(
                { ...sp, cursor: page.next_cursor, p: String(pageNo + 1) },
                "dir",
                undefined,
              )
            : null;
        if (!prevHref && !nextHref) return null;
        return (
          <nav className="flex items-center justify-center gap-3 py-1">
            {prevHref && (
              <a href={prevHref} className={PILL_CLS}>
                {dict.common.prevPage}
              </a>
            )}
            <span className="text-sm text-sub">
              {fmt(dict.common.pageX, { n: pageNo })}
              {" · "}
              {fmt(dict.common.totalItems, {
                n: page.total_estimate ?? 0,
              })}
            </span>
            {nextHref && (
              <a href={nextHref} className={PILL_CLS}>
                {dict.common.nextPage}
              </a>
            )}
          </nav>
        );
      })()}
    </div>
  );
}
