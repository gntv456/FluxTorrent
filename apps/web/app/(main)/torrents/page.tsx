import { EmptyTorrents } from "@/components/torrent";
import { TorrentTr } from "@/components/torrent-table";
import { api, paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

interface SectionDictRow { id: number; kind: string; name: string; sort: number }
interface SectionKindMeta { kind: string; label: string; sort: number }

/** 匿名公开接口统一抓取（api.get 自带 SSR 内网直连 API_SERVER_URL；失败时隐藏对应筛选区） */
async function loadPublic<T>(path: string): Promise<T | null> {
  try {
    return await api.get<T>(path);
  } catch {
    return null;
  }
}

/** 表头排序切换：当前列降序 → 升序（_asc）→ 取消；其他列 → 降序 */
function toggleSort(cur: string | undefined, key: string): string | undefined {
  if (cur === key) return `${key}_asc`;
  if (cur === `${key}_asc`) return undefined;
  return key;
}

/** 在现有参数上增量修改，保留其余筛选（修复翻页丢参数） */
function withParam(
  sp: Record<string, string | undefined>,
  key: string,
  value: string | undefined,
): string {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(sp)) {
    if (v !== undefined && v !== "" && k !== key) qs.set(k, v);
  }
  if (value !== undefined && value !== "") qs.set(key, value);
  const s = qs.toString();
  return s ? `/torrents?${s}` : "/torrents";
}

/** 种子页（参考站 torrents.php 复刻）：
 *  搜索盒（范围/关键字/匹配模式/给我搜/高级搜索折叠）+ 分类 chip + 九列 colhead 图标表头表格
 *  0088：分类走 site-profile（与后台同步）；高级搜索 = 多选分类 + 排序 + 标签 + 九维动态筛选 */
export default async function TorrentsPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const spRaw = await searchParams;
  // 归一化：重复参数（多选分类 checkbox）→ 逗号串；单值原样
  const sp: Record<string, string | undefined> = Object.fromEntries(
    Object.entries(spRaw).map(([k, v]) => [k, Array.isArray(v) ? v.join(",") : v]),
  );
  const { dict } = await getDict();
  const [profile, secDict, tagDict] = await Promise.all([
    loadPublic<{
      categories: { id: number; name: string }[];
      metadata_sources?: string[];
    }>("/api/v1/site-profile"),
    loadPublic<Record<string, SectionDictRow[]> & { kinds?: SectionKindMeta[] }>(
      "/api/v1/section-dict",
    ),
    loadPublic<{ id: number; name: string; kind: string }[] | [number, string, string][]>(
      "/api/v1/tags-dict",
    ),
  ]);
  const kinds: SectionKindMeta[] = secDict?.kinds ?? [];
  const dimKinds = kinds.filter((k) => (secDict?.[k.kind]?.length ?? 0) > 0);
  const tags = (tagDict ?? []).map((r) =>
    Array.isArray(r) ? { id: r[0], name: r[1], kind: r[2] } : r,
  );
  // 多维筛选参数转发（sec_{kind} → 后端通用解析）：不转发则筛选静默失效
  const secParams: Record<string, string> = {};
  for (const [k, v] of Object.entries(sp)) {
    if (k.startsWith("sec_") && v) secParams[k] = v;
  }
  // 半旧会话（cookie 无 token）或后端抖动时降级为空列表，页面骨架仍可用
  const page = await paged<TorrentListItem>("/api/v1/torrents", {
    limit: 20,
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
    tag_id: sp.tag_id ? Number(sp.tag_id) : undefined,
    cursor: sp.cursor,
    ...secParams,
  }).catch(() => ({
    items: [] as TorrentListItem[],
    next_cursor: null,
    total_estimate: 0,
  }));

  // 分类以 site-profile 为准（后台可改，与站型包同步）；接口失败回落 i18n 字典
  const categories = (profile?.categories?.length
    ? profile.categories.map((c) => ({ id: c.id, label: c.name }))
    : dict.torrents.categories.slice(1).map((label, i) => ({ id: i + 1, label })));
  // 多选分类：URL 里同名参数（checkbox 多选），解析去重
  const selectedCats = new Set(
    (sp.category_id ?? "")
      .split(",")
      .filter(Boolean)
      .map(Number),
  );

  return (
    <div className="flex flex-col gap-3">
      <h1 className="sr-only">{dict.torrents.title}</h1>

      {/* 搜索盒（参考站 torrent-search-box 同构：范围+关键字+匹配模式 / 给我搜 / 高级搜索折叠） */}
      <form action="/torrents" method="get" className="torrent-search-form">
        <table className="searchbox torrent-search-box">
          <tbody className="torrent-search-box__quick">
            <tr>
              <td className="rowfollow torrent-search-box__quick-fields">
                <div className="torrent-search-box__primary">
                  <div className="torrent-search-box__search-line">
                    <label className="torrent-search-box__scope">
                      <span className="torrent-search-box__visually-hidden">
                        {dict.torrents2.scope}：
                      </span>
                      <select
                        name="search_area"
                        defaultValue={sp.search_area ?? "0"}
                        aria-label={dict.torrents2.scope}
                      >
                        <option value="0">{dict.torrents2.areaTitle}</option>
                        <option value="1">{dict.torrents2.areaDescr}</option>
                        <option value="3">{dict.torrents2.areaUploader}</option>
                        {/* IMDb 范围随站点元数据源显隐（与上传页条目输入同口径） */}
                        {(!profile || (profile.metadata_sources ?? []).includes("imdb")) && (
                          <option value="4">IMDb</option>
                        )}
                      </select>
                    </label>
                    <div className="torrent-search-box__keyword">
                      <input
                        id="searchinput"
                        name="search"
                        type="text"
                        defaultValue={sp.search}
                        placeholder={dict.torrents2.keywordPh}
                        autoComplete="off"
                      />
                    </div>
                    <label className="torrent-search-box__mode">
                      <span className="torrent-search-box__visually-hidden">
                        {dict.torrents2.mode}：
                      </span>
                      <select
                        name="search_mode"
                        defaultValue={sp.search_mode ?? "0"}
                        aria-label={dict.torrents2.mode}
                      >
                        <option value="0">{dict.torrents2.modeAnd}</option>
                        <option value="2">{dict.torrents2.modeExact}</option>
                      </select>
                    </label>
                  </div>
                  <button type="submit" className="baozi-button torrent-search-box__submit-button">
                    {dict.torrents2.searchBtn}
                  </button>
                  {/* 高级搜索折叠区（原生 details，与旧站 aria-expanded 折叠一致） */}
                  <details className="torrent-search-box__advanced-toggle">
                    <summary>
                      <span>{dict.torrents2.advanced}</span>
                      <span className="torrent-search-box__chevron" aria-hidden="true" />
                    </summary>
                    <div className="torrent-search-box__advanced-content">
                      <div className="torrent-search-box__taxonomy-grid">
                        <fieldset>
                          <legend>{dict.torrents2.catLegend}</legend>
                          <div className="torrent-search-box__cat-checks">
                            {/* 分类与后台设置同步（0088：site-profile 实时分类，多选） */}
                            {categories.map((c) => (
                              <label key={c.id} className="torrent-search-box__cat-item">
                                <input
                                  type="checkbox"
                                  name="category_id"
                                  value={c.id}
                                  defaultChecked={selectedCats.has(c.id)}
                                />
                                {c.label}
                              </label>
                            ))}
                          </div>
                        </fieldset>
                        <fieldset>
                          <legend>{dict.torrents2.sort ?? "排序"}</legend>
                          <label className="torrent-search-box__cat-item">
                            <select name="sort" defaultValue={sp.sort ?? ""} aria-label={dict.torrents2.sort ?? "排序"}>
                              <option value="">{dict.torrents2.sortDefault ?? "默认（最新）"}</option>
                              <option value="seeders">{dict.torrents2.sortSeeders ?? "做种最多"}</option>
                              <option value="size">{dict.torrents2.sortSize ?? "体积最大"}</option>
                              <option value="completed">{dict.torrents2.sortCompleted ?? "完成最多"}</option>
                            </select>
                          </label>
                          <label className="torrent-search-box__cat-item">
                            <select name="tag_id" defaultValue={sp.tag_id ?? ""} aria-label={dict.torrTags2?.title ?? "标签"}>
                              <option value="">{dict.torrTags2?.title ?? "标签"}</option>
                              {tags
                                .filter((t) => t.kind !== "official")
                                .map((t) => (
                                  <option key={t.id} value={t.id}>{t.name}</option>
                                ))}
                            </select>
                          </label>
                        </fieldset>
                        <fieldset>
                          <legend>{dict.torrents2.aliveLegend ?? "存活"}</legend>
                          {[
                            { v: "", label: dict.torrents2.aliveAll ?? "全部" },
                            { v: "2", label: dict.torrents2.aliveDead ?? "包括断种" },
                            { v: "1", label: dict.torrents2.aliveAliveOnly ?? "仅活种" },
                          ].map((o) => (
                            <label key={o.v} className="torrent-search-box__cat-item">
                              <input
                                type="radio"
                                name="alive"
                                value={o.v}
                                defaultChecked={(sp.alive ?? "") === o.v}
                              />
                              {o.label}
                            </label>
                          ))}
                        </fieldset>
                        <fieldset>
                          <legend>{dict.torrents2.statusLegend ?? "种子状态"}</legend>
                          {[
                            { v: "", label: dict.torrents2.statusAll ?? "全部" },
                            { v: "seeding", label: dict.torrents2.stSeeding ?? "当前做种" },
                            { v: "leeching", label: dict.torrents2.stLeeching ?? "当前下载" },
                            { v: "completed", label: dict.torrents2.stCompleted ?? "完成种子" },
                            { v: "incomplete", label: dict.torrents2.stIncomplete ?? "未完成种子" },
                            { v: "notseeding", label: dict.torrents2.stNotSeeding ?? "未做种种子" },
                          ].map((o) => (
                            <label key={o.v} className="torrent-search-box__cat-item">
                              <input
                                type="radio"
                                name="status"
                                value={o.v}
                                defaultChecked={(sp.status ?? "") === o.v}
                              />
                              {o.label}
                            </label>
                          ))}
                        </fieldset>
                        <fieldset>
                          <legend>{dict.torrents2.approvalLegend ?? "审核状态"}</legend>
                          {[
                            { v: "", label: dict.torrents2.approvalAll ?? "全部" },
                            { v: "1", label: dict.torrents2.approvalPassed ?? "通过" },
                            { v: "2", label: dict.torrents2.approvalRejected ?? "拒绝" },
                          ].map((o) => (
                            <label key={o.v} className="torrent-search-box__cat-item">
                              <input
                                type="radio"
                                name="approval"
                                value={o.v}
                                defaultChecked={(sp.approval ?? "") === o.v}
                              />
                              {o.label}
                            </label>
                          ))}
                        </fieldset>
                        <fieldset>
                          <legend>{dict.torrents2.officialLegend}</legend>
                          <label className="torrent-search-box__cat-item">
                            <input
                              type="checkbox"
                              name="official"
                              value="1"
                              defaultChecked={sp.official === "1"}
                            />
                            {dict.torrents2.officialOnly}
                          </label>
                        </fieldset>
                        {dimKinds.length > 0 && (
                          <fieldset>
                            <legend>{dict.torrents2.dimLegend ?? "多维筛选"}</legend>
                            <div className="torrent-search-box__cat-checks">
                              {dimKinds.map((k) => (
                                <div key={k.kind} className="torrent-search-box__dim-group">
                                  <span className="torrent-search-box__dim-label">{k.label}</span>
                                  {(secDict?.[k.kind] ?? []).map((d) => (
                                    <label key={d.id} className="torrent-search-box__cat-item">
                                      <input
                                        type="checkbox"
                                        name={`sec_${k.kind}`}
                                        value={d.id}
                                        defaultChecked={(sp[`sec_${k.kind}`] ?? "")
                                          .split(",")
                                          .includes(String(d.id))}
                                      />
                                      {d.name}
                                    </label>
                                  ))}
                                </div>
                              ))}
                            </div>
                          </fieldset>
                        )}
                      </div>
                    </div>
                  </details>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </form>

      <span className="num text-xs text-sub">
        {fmt(dict.torrents.total, { n: page.total_estimate })}
      </span>

      {/* 种子九列表格（参考站 colhead 图标表头） */}
      {page.items.length === 0 ? (
        <EmptyTorrents />
      ) : (
        <div className="baozi-wide-table-scroll" role="region" aria-label={dict.torrents.title}>
          <table className="nexus-table torrents-table">
            <thead>
              <tr>
                <th className="w-12">{dict.torrents.colType}</th>
                <th className="w-16" aria-label="封面" />
                <th>
                  <a href={withParam(sp, "sort", undefined)}>{dict.torrents.colTitle}</a>
                </th>
                {/* 表头点击排序（NP colhead 口径）：同列再点反转升降序 */}
                <th className="w-16" title={dict.torrents.colComments}>
                  <a href={withParam(sp, "sort", toggleSort(sp.sort, "comments"))}>💬</a>
                </th>
                <th className="w-20" title={dict.torrents.alive}>
                  ⏱
                </th>
                <th className="w-20" title={dict.torrents.colSize}>
                  <a href={withParam(sp, "sort", toggleSort(sp.sort, "size"))}>💾</a>
                </th>
                <th className="w-16" title={dict.torrents.colSeeders}>
                  <a href={withParam(sp, "sort", toggleSort(sp.sort, "seeders"))}>🌱</a>
                </th>
                <th className="w-16" title={dict.torrents.colLeechers}>
                  ⬇️
                </th>
                <th className="w-16" title={dict.torrents.colCompleted}>
                  <a href={withParam(sp, "sort", toggleSort(sp.sort, "completed"))}>✅</a>
                </th>
                <th className="w-24">{dict.torrents.colActions}</th>
              </tr>
            </thead>
            <tbody>
              {page.items.map((t) => (
                <TorrentTr key={t.id} t={t} />
              ))}
            </tbody>
          </table>
        </div>
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
