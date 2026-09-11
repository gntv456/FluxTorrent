import { EmptyTorrents } from "@/components/torrent";
import { TorrentTr } from "@/components/torrent-table";
import { paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

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

/** 种子页（包子站 torrents.php 复刻）：
 *  搜索盒（范围/关键字/匹配模式/给我搜/高级搜索折叠）+ 分类 chip + 九列 colhead 图标表头表格 */
export default async function TorrentsPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | undefined>>;
}) {
  const sp = await searchParams;
  const { dict } = await getDict();
  // 半旧会话（cookie 无 token）或后端抖动时降级为空列表，页面骨架仍可用
  const page = await paged<TorrentListItem>("/api/v1/torrents", {
    limit: 20,
    category_id: sp.category_id ? Number(sp.category_id) : undefined,
    medium_id: sp.medium_id ? Number(sp.medium_id) : undefined,
    grade_id: sp.grade_id ? Number(sp.grade_id) : undefined,
    official: sp.official ? sp.official === "1" : undefined,
    include_dead: sp.include_dead === "1",
    search: sp.search,
    sort: sp.sort,
    tag_id: sp.tag_id ? Number(sp.tag_id) : undefined,
    cursor: sp.cursor,
  }).catch(() => ({
    items: [] as TorrentListItem[],
    next_cursor: null,
    total_estimate: 0,
  }));

  // 字典分类/媒介数组按下标对齐：index 0 = 全部，1..n = 对应 id
  const categories = dict.torrents.categories.map((label, i) => ({
    id: i === 0 ? undefined : i,
    label,
  }));
  const media = dict.torrents.media.map((label, i) => ({
    id: i === 0 ? undefined : i,
    label,
  }));
  const currentCategory = sp.category_id ? Number(sp.category_id) : undefined;
  const currentMedium = sp.medium_id ? Number(sp.medium_id) : undefined;
  const currentGrade = sp.grade_id !== undefined && sp.grade_id !== "" ? Number(sp.grade_id) : undefined;

  return (
    <div className="flex flex-col gap-3">
      <h1 className="sr-only">{dict.torrents.title}</h1>

      {/* 搜索盒（包子站 torrent-search-box 同构：范围+关键字+匹配模式 / 给我搜 / 高级搜索折叠） */}
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
                      <select name="search_area" aria-label={dict.torrents2.scope}>
                        <option value="0">{dict.torrents2.areaTitle}</option>
                        <option value="1">{dict.torrents2.areaDescr}</option>
                        <option value="3">{dict.torrents2.areaUploader}</option>
                        <option value="4">IMDb</option>
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
                      <select name="search_mode" aria-label={dict.torrents2.mode}>
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
                            {categories.slice(1).map((c) => (
                              <label key={c.label} className="torrent-search-box__cat-item">
                                <input
                                  type="checkbox"
                                  name="category_id"
                                  value={c.id}
                                  defaultChecked={
                                    currentCategory !== undefined && currentCategory === c.id
                                  }
                                />
                                {c.label}
                              </label>
                            ))}
                          </div>
                        </fieldset>
                        <fieldset>
                          <legend>{dict.torrents2.deadLegend}</legend>
                          <label className="torrent-search-box__cat-item">
                            <input
                              type="checkbox"
                              name="include_dead"
                              value="1"
                              defaultChecked={sp.include_dead === "1"}
                            />
                            {dict.torrents2.inclDead}
                          </label>
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

      {/* 筛选 Chip（移动端横向滚动）—— 增量参数互不覆盖 */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {categories.map((c) => (
          <a
            key={c.label}
            href={withParam(sp, "category_id", c.id ? String(c.id) : undefined)}
            aria-current={currentCategory === c.id ? "true" : undefined}
            data-active={currentCategory === c.id ? "true" : undefined}
            className="filter-chip"
          >
            {c.label}
          </a>
        ))}
      </div>
      <div className="flex gap-2 overflow-x-auto pb-1">
        {media.map((m) => (
          <a
            key={m.label}
            href={withParam(sp, "medium_id", m.id ? String(m.id) : undefined)}
            aria-current={currentMedium === m.id ? "true" : undefined}
            data-active={currentMedium === m.id ? "true" : undefined}
            className="filter-chip"
          >
            {m.label}
          </a>
        ))}
      </div>
      {/* 标签筛选（T-04：tag_dict 1-5） */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {(dict.torrTags2?.filterRow ?? ["全部", "官方", "免费", "官种", "合集", "带答案"]).map((label, i) => (
          <a
            key={label}
            href={withParam(sp, "tag_id", i === 0 ? undefined : String(i))}
            aria-current={(sp.tag_id ? Number(sp.tag_id) : undefined) === (i === 0 ? undefined : i) ? "true" : undefined}
            data-active={(sp.tag_id ? Number(sp.tag_id) : undefined) === (i === 0 ? undefined : i) ? "true" : undefined}
            className="filter-chip"
          >
            {label}
          </a>
        ))}
      </div>
      {/* 学段筛选（grades id 0-12 与字典下标 i=id+1 对齐） */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {dict.torrents.grades.map((label, gid) => (
          <a
            key={label}
            href={withParam(sp, "grade_id", gid === 0 ? undefined : String(gid - 1))}
            aria-current={currentGrade === (gid === 0 ? undefined : gid - 1) ? "true" : undefined}
            data-active={currentGrade === (gid === 0 ? undefined : gid - 1) ? "true" : undefined}
            className="filter-chip"
          >
            {label}
          </a>
        ))}
      </div>

      {/* 排序（旧站 torrents.php 口径） */}
      <div className="flex gap-2 overflow-x-auto pb-1">
        {(
          [
            [undefined, dict.torrents.sortCreated],
            ["seeders", dict.torrents.sortSeeders],
            ["size", dict.torrents.sortSize],
            ["completed", dict.torrents.sortCompleted],
          ] as [string | undefined, string][]
        ).map(([value, label]) => (
          <a
            key={label}
            href={withParam(sp, "sort", value)}
            aria-current={(sp.sort ?? undefined) === value ? "true" : undefined}
            data-active={(sp.sort ?? undefined) === value ? "true" : undefined}
            className="filter-chip"
          >
            {label}
          </a>
        ))}
      </div>

      {/* 种子九列表格（包子站 colhead 图标表头） */}
      {page.items.length === 0 ? (
        <EmptyTorrents />
      ) : (
        <div className="baozi-wide-table-scroll" role="region" aria-label={dict.torrents.title}>
          <table className="nexus-table torrents-table">
            <thead>
              <tr>
                <th className="w-12">{dict.torrents.colType}</th>
                <th>
                  <a href={withParam(sp, "sort", undefined)}>{dict.torrents.colTitle}</a>
                </th>
                <th className="w-16" title={dict.torrents.colComments}>
                  💬
                </th>
                <th className="w-20" title={dict.torrents2.colAlive}>
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
                <th className="w-28">{dict.torrents.colOwner}</th>
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
