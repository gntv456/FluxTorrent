import { EmptyTorrents } from "@/components/torrent";
import { api, paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import { TorrentsSearchBox } from "./_parts/torrents-search-box";
import { TorrentsTable } from "./_parts/torrents-table";
import { TorrentCards, TorrentPosters } from "./_parts/torrents-cards";
import { TorrentsViewSwitch } from "./_parts/torrents-view-switch";
import { TorrentsHotkeys } from "./_parts/torrents-hotkeys";
import { TorrentHoverPreview } from "@/components/torrent-hover-preview";
import { buildTorrentChips } from "./_parts/torrents-chips";
import {
  loadPublic,
  parsePageSize,
  parseView,
  toggleSort,
  withParam,
  type SectionDictRow,
  type SectionKindMeta,
} from "./_parts/torrents-utils";

export const dynamic = "force-dynamic";

/** 种子页（参考站 torrents.php 复刻 + 0118 高级搜索重排）：
 *  常驻搜索行（范围/关键字/匹配/搜索）+ 折叠高级面板按语义分组（基础筛选 / 数值与时间 /
 *  发布者与状态 / 标签与排序 / 多维筛选），每组内卡片网格；已选条件摘要条置顶可逐个移除。
 *  搜索盒 / 表格 / chips 拼装 / URL 工具拆至 ./_parts/（300 行门禁）。 */
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
  const { dict } = await getDict();
  // 列表形态（方案阶段二）：视图与每页条数都走 URL，未知值回落默认（table / 20）
  const view = parseView(sp.view);
  const pageSize = parsePageSize(sp.limit);
  const [profile, secDict, tagDict] = await Promise.all([
    loadPublic<{
      categories: { id: number; name: string }[];
      metadata_sources?: string[];
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
  const kinds: SectionKindMeta[] = secDict?.kinds ?? [];
  const dimKinds = kinds.filter((k) => (secDict?.[k.kind]?.length ?? 0) > 0);
  // 0160 起返回 { tags, groups }；旧形态（裸数组）兼容
  const tagRows = tagDict && !Array.isArray(tagDict) ? tagDict.tags : (tagDict ?? []);
  const tags = (tagRows ?? []).map((r) =>
    Array.isArray(r) ? { id: r[0], name: r[1], kind: r[2] } : r,
  );
  // 平行组别数组（0160 P2）：筛选面板按 attribute/content 分两行渲染
  const tagGroups =
    tagDict && !Array.isArray(tagDict) ? (tagDict.groups ?? []) : [];
  // 多维筛选参数转发（sec_{kind} → 后端通用解析）：不转发则筛选静默失效
  const secParams: Record<string, string> = {};
  for (const [k, v] of Object.entries(sp)) {
    if (k.startsWith("sec_") && v) secParams[k] = v;
  }
  // 半旧会话（cookie 无 token）或后端抖动时降级为空列表，页面骨架仍可用
  // 半旧会话（cookie 无 token）或后端抖动时降级为空列表，页面骨架仍可用；
  // 但要把「失败」与「确实没有结果」区分开（方案 P0-6：此前两者都渲染成空态）
  let loadFailed = false;
  const page = await paged<TorrentListItem>("/api/v1/torrents", {
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
    anonymous: sp.anonymous || undefined,
    ...secParams,
  }).catch(() => {
    loadFailed = true;
    return {
      items: [] as TorrentListItem[],
      next_cursor: null,
      total_estimate: 0,
    };
  });

  // 分类以 site-profile 为准（后台可改，与站型包同步）；接口失败回落 i18n 字典
  const categories = profile?.categories?.length
    ? profile.categories.map((c) => ({ id: c.id, label: c.name }))
    : dict.torrents.categories
        .slice(1)
        .map((label, i) => ({ id: i + 1, label }));
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
      <h1 className="sr-only">{dict.torrents.title}</h1>

      {/* 搜索盒：常驻一行 + 折叠高级面板（0105 重排） */}
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

      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="num text-xs text-sub">
          {fmt(dict.torrents.total, { n: page.total_estimate })}
        </span>
        <TorrentsViewSwitch
          dict={dict}
          sp={sp}
          view={view}
          pageSize={pageSize}
          withParam={withParam}
        />
      </div>
      {/* 键盘快捷键（/ 搜索、j/k 移动、Enter 打开）：挂载后短暂提示，8s 自动消失 */}
      <TorrentsHotkeys />
      {/* 列表 hover 预览卡（阶段三）：事件委托整页生效 */}
      <TorrentHoverPreview />

      {/* 三种形态共享同一份数据：表格（信息密度）/ 卡片（浏览）/ 海报墙（视觉） */}
      {page.items.length === 0 ? (
        loadFailed ? (
          <p className="py-8 text-center text-sm text-sub" role="status">
            {dict.common.loadFailed}
            <a
              href={withParam(sp, "cursor", undefined)}
              className="ml-2 text-sky underline"
            >
              {dict.common.retry}
            </a>
          </p>
        ) : (
          <EmptyTorrents />
        )
      ) : view === "card" ? (
        <TorrentCards items={page.items} dict={dict} />
      ) : view === "poster" ? (
        <TorrentPosters items={page.items} dict={dict} />
      ) : (
        <TorrentsTable
          dict={dict}
          sp={sp}
          items={page.items}
          withParam={withParam}
          toggleSort={toggleSort}
        />
      )}

      {/* RSS 订阅入口（NP 列表头 RSS 图标口径）：携带当前关键字/分类跳转订阅页 */}
      <div className="flex justify-end">
        <a
          href={`/getrss?keyword=${encodeURIComponent(sp.search ?? "")}&cats=${sp.category_id ?? ""}`}
          className="text-xs text-sky hover:underline"
        >
          📡 {dict.getrss.title}
        </a>
      </div>

      {page.next_cursor && (
        <a
          href={withParam(sp, "cursor", page.next_cursor)}
          className="mx-auto min-h-[44px] flex items-center rounded-full border border-line bg-[var(--surface-card)] px-6 text-sm text-sky-deep"
        >
          {dict.common.nextPage}
        </a>
      )}
    </div>
  );
}
