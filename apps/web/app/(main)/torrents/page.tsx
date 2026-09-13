import { EmptyTorrents } from "@/components/torrent";
import { TorrentTr } from "@/components/torrent-table";
import { paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

interface SectionDictRow { id: number; kind: string; name: string; sort: number }

/** 第八轮 Section 多维：筛选下拉字典（匿名接口；失败时隐藏筛选区） */
async function loadSectionDict(): Promise<Record<string, SectionDictRow[]>> {
  const base = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
  try {
    const res = await fetch(`${base}/api/v1/section-dict`, { cache: "no-store" });
    const j = await res.json();
    return j?.code === 0 ? (j.data as Record<string, SectionDictRow[]>) : {};
  } catch {
    return {};
  }
}

const SEC_KINDS: [string, string][] = [
  ["codec", "编码"],
  ["audio_codec", "音频编码"],
  ["standard", "规格"],
  ["team", "制作组"],
  ["source", "来源"],
  ["processing", "处理工艺"],
];

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
  const secDict = await loadSectionDict();
  const hasSecDict = SEC_KINDS.some(([k]) => (secDict[k]?.length ?? 0) > 0);
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
    sec_codec: sp.sec_codec ? Number(sp.sec_codec) : undefined,
    sec_audio_codec: sp.sec_audio_codec ? Number(sp.sec_audio_codec) : undefined,
    sec_standard: sp.sec_standard ? Number(sp.sec_standard) : undefined,
    sec_team: sp.sec_team ? Number(sp.sec_team) : undefined,
    sec_source: sp.sec_source ? Number(sp.sec_source) : undefined,
    sec_processing: sp.sec_processing ? Number(sp.sec_processing) : undefined,
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
  const currentCategory = sp.category_id ? Number(sp.category_id) : undefined;

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
                        {hasSecDict && (
                          <fieldset>
                            <legend>多维筛选</legend>
                            <div className="torrent-search-box__cat-checks">
                              {SEC_KINDS.map(([k, label]) => (
                                (secDict[k]?.length ?? 0) > 0 && (
                                  <label key={k} className="torrent-search-box__cat-item">
                                    <select name={`sec_${k}`} defaultValue={sp[`sec_${k}`] ?? ""} aria-label={label}>
                                      <option value="">{label}</option>
                                      {secDict[k].map((d) => (
                                        <option key={d.id} value={d.id}>{d.name}</option>
                                      ))}
                                    </select>
                                  </label>
                                )
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

      {/* 种子九列表格（包子站 colhead 图标表头） */}
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
