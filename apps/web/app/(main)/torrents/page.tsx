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

/** 种子页（参考站 torrents.php 复刻 + 0118 高级搜索重排）：
 *  常驻搜索行（范围/关键字/匹配/搜索）+ 折叠高级面板按语义分组（基础筛选 / 数值与时间 /
 *  发布者与状态 / 标签与排序 / 多维筛选），每组内卡片网格；已选条件摘要条置顶可逐个移除。 */
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
    // 0118 补齐（下载数/完成数区间 / 匿名发布 / 排序视图）
    min_leechers: sp.min_leechers || undefined,
    max_leechers: sp.max_leechers || undefined,
    min_completed: sp.min_completed || undefined,
    max_completed: sp.max_completed || undefined,
    anonymous: sp.anonymous || undefined,
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
  const catName = (id: string) =>
    categories.find((c) => String(c.id) === id)?.label ?? id;

  // ===== 已选条件摘要（每个 chip 可单独移除；「排序」不算筛选条件） =====
  const chips: { key: string; text: string; href: string }[] = [];
  const chip = (key: string, text: string) => {
    chips.push({ key, text, href: withParam(sp, key, undefined) });
  };
  const areaLabels: Record<string, string> = {
    "0": dict.torrents2.areaTitle,
    "1": dict.torrents2.areaDescr,
    "3": dict.torrents2.areaUploader,
    "4": "IMDb",
  };
  if (sp.search) chip("search", `${dict.torrents2.keywordPh}：${sp.search}`);
  if (sp.search_area && sp.search_area !== "0") {
    chip("search_area", `${dict.torrents2.scope}：${areaLabels[sp.search_area] ?? sp.search_area}`);
  }
  if (sp.search_mode === "2") chip("search_mode", dict.torrents2.modeExact);
  if (sp.category_id) {
    const names = sp.category_id.split(",").filter(Boolean).map(catName);
    chip(
      "category_id",
      `${dict.torrents2.catLegend}：${names.slice(0, 3).join("、")}${names.length > 3 ? ` +${names.length - 3}` : ""}`,
    );
  }
  if (sp.alive === "1") chip("alive", dict.torrents2.aliveAliveOnly);
  if (sp.alive === "2") chip("alive", dict.torrents2.aliveDead);
  if (sp.status) {
    const stLabels: Record<string, string> = {
      seeding: dict.torrents2.stSeeding,
      leeching: dict.torrents2.stLeeching,
      completed: dict.torrents2.stCompleted,
      incomplete: dict.torrents2.stIncomplete,
      notseeding: dict.torrents2.stNotSeeding,
    };
    chip("status", `${dict.torrents2.statusLegend}：${stLabels[sp.status] ?? sp.status}`);
  }
  if (sp.approval === "1") chip("approval", `${dict.torrents2.approvalLegend}：${dict.torrents2.approvalPassed}`);
  if (sp.approval === "2") chip("approval", `${dict.torrents2.approvalLegend}：${dict.torrents2.approvalRejected}`);
  if (sp.official === "1") chip("official", dict.torrents2.officialOnly);
  if (sp.mine === "1") chip("mine", dict.torrents2.mineOnly);
  if (sp.tag_id) {
    const t = tags.find((x) => String(x.id) === sp.tag_id);
    chip("tag_id", `${dict.torrents2.tagLabel}：${t?.name ?? sp.tag_id}`);
  }
  if (sp.promo) {
    const prLabels: Record<string, string> = {
      free: dict.torrents2.promoFree,
      x2: dict.torrents2.promoX2,
      half: dict.torrents2.promoHalf,
      any: dict.torrents2.promoHas,
      none: dict.torrents2.promoNone,
    };
    chip("promo", `${dict.torrents2.promoLegend}：${prLabels[sp.promo] ?? sp.promo}`);
  }
  if (sp.size_min) chip("size_min", `${dict.torrents2.sizeLegend} ≥ ${sp.size_min}`);
  if (sp.size_max) chip("size_max", `${dict.torrents2.sizeLegend} ≤ ${sp.size_max}`);
  if (sp.min_seeders) chip("min_seeders", `${dict.torrents2.seedersLegend} ≥ ${sp.min_seeders}`);
  if (sp.max_seeders) chip("max_seeders", `${dict.torrents2.seedersLegend} ≤ ${sp.max_seeders}`);
  if (sp.min_leechers) chip("min_leechers", `${dict.torrents2.leechersLegend} ≥ ${sp.min_leechers}`);
  if (sp.max_leechers) chip("max_leechers", `${dict.torrents2.leechersLegend} ≤ ${sp.max_leechers}`);
  if (sp.min_completed) chip("min_completed", `${dict.torrents2.completedLegend} ≥ ${sp.min_completed}`);
  if (sp.max_completed) chip("max_completed", `${dict.torrents2.completedLegend} ≤ ${sp.max_completed}`);
  if (sp.date_from) chip("date_from", `${dict.torrents2.dateLegend} ≥ ${sp.date_from}`);
  if (sp.date_to) chip("date_to", `${dict.torrents2.dateLegend} ≤ ${sp.date_to}`);
  if (sp.owner) chip("owner", `${dict.torrents2.ownerLegend}：${sp.owner}`);
  if (sp.exclude) chip("exclude", `${dict.torrents2.excludeLegend}：${sp.exclude}`);
  if (sp.anonymous === "1") chip("anonymous", dict.torrents2.anonOnly);
  if (sp.anonymous === "2") chip("anonymous", dict.torrents2.anonNamed);
  for (const k of dimKinds) {
    const raw = sp[`sec_${k.kind}`];
    if (!raw) continue;
    const names = raw
      .split(",")
      .filter(Boolean)
      .map((id) => (secDict?.[k.kind] ?? []).find((d) => String(d.id) === id)?.name ?? id);
    chip(`sec_${k.kind}`, `${k.label}：${names.join("、")}`);
  }
  const advancedOpen = chips.length > 0;
  const nonSearchChips = chips.filter((c) => c.key !== "search").length;

  /** 选项 chips（checkbox 多选 / radio 单选共用渲染） */
  const chipLabel = (
    name: string,
    value: string,
    text: string,
    checked: boolean,
    type: "checkbox" | "radio" = "checkbox",
  ) => (
    <label key={`${name}-${value}`} className="tsb-chip">
      <input type={type} name={name} value={value} defaultChecked={checked} />
      <span>{text}</span>
    </label>
  );

  /** 行内数值区间（min/max 一对输入；name 前缀 + legend 拼装 aria） */
  const rangeInputs = (
    legend: string,
    name: string,
    minPh: string,
    maxPh: string,
    minVal?: string,
    maxVal?: string,
    date = false,
  ) => (
    <div className={date ? "tsb-range tsb-range--col" : "tsb-range"}>
      <input
        className="tsb-num"
        name={`${name}_min`}
        type={date ? "date" : "number"}
        min={date ? undefined : 0}
        defaultValue={minVal}
        placeholder={minPh}
        aria-label={`${legend} ${minPh}`}
      />
      <span className="tsb-range__sep" aria-hidden="true">–</span>
      <input
        className="tsb-num"
        name={`${name}_max`}
        type={date ? "date" : "number"}
        min={date ? undefined : 0}
        defaultValue={maxVal}
        placeholder={maxPh}
        aria-label={`${legend} ${maxPh}`}
      />
    </div>
  );

  return (
    <div className="flex flex-col gap-3">
      <h1 className="sr-only">{dict.torrents.title}</h1>

      {/* 搜索盒：常驻一行 + 折叠高级面板（0105 重排） */}
      <form action="/torrents" method="get" className="torrent-search-box">
        {/* ── 常驻搜索行 ── */}
        <div className="tsb-bar">
          <label className="tsb-field tsb-field--scope">
            <span className="tsb-field__label">{dict.torrents2.scope}</span>
            <select name="search_area" defaultValue={sp.search_area ?? "0"} aria-label={dict.torrents2.scope}>
              <option value="0">{dict.torrents2.areaTitle}</option>
              <option value="1">{dict.torrents2.areaDescr}</option>
              <option value="3">{dict.torrents2.areaUploader}</option>
              {/* IMDb 范围随站点元数据源显隐（与上传页条目输入同口径） */}
              {(!profile || (profile.metadata_sources ?? []).includes("imdb")) && (
                <option value="4">IMDb</option>
              )}
            </select>
          </label>
          <input
            id="searchinput"
            name="search"
            type="text"
            className="tsb-input"
            defaultValue={sp.search}
            placeholder={dict.torrents2.keywordPh}
            autoComplete="off"
          />
          <label className="tsb-field tsb-field--mode">
            <span className="tsb-field__label">{dict.torrents2.mode}</span>
            <select name="search_mode" defaultValue={sp.search_mode ?? "0"} aria-label={dict.torrents2.mode}>
              <option value="0">{dict.torrents2.modeAnd}</option>
              <option value="2">{dict.torrents2.modeExact}</option>
            </select>
          </label>
          <button type="submit" className="baozi-button tsb-submit">
            {dict.torrents2.searchBtn}
          </button>
        </div>

        {/* ── 已选条件摘要（可单个移除） ── */}
        {chips.length > 0 && (
          <div className="tsb-active">
            <span className="tsb-active__title">{dict.torrents2.activeTitle}</span>
            <div className="tsb-active__list">
              {chips.map((c) => (
                <a key={c.key} href={c.href} className="tsb-active__chip" title={dict.torrents2.removeFilter}>
                  <span>{c.text}</span>
                  <span className="tsb-active__x" aria-hidden="true">×</span>
                </a>
              ))}
            </div>
            <a href="/torrents" className="tsb-active__clear">{dict.torrents2.clearAll}</a>
          </div>
        )}

        {/* ── 高级搜索（原生 details 折叠；有筛选条件时默认展开；0118 分组重排） ── */}
        <details className="tsb-adv" open={advancedOpen}>
          <summary className="tsb-adv__summary">
            <span className="tsb-adv__icon" aria-hidden="true">⚙</span>
            <span>{dict.torrents2.advanced}</span>
            {nonSearchChips > 0 && <span className="tsb-adv__badge">{nonSearchChips}</span>}
            <span className="tsb-chevron" aria-hidden="true" />
          </summary>
          <div className="tsb-adv__body">
            <p className="tsb-adv__hint">{dict.torrents2.advHint}</p>

            <div className="tsb-group">
              {/* ── 基础筛选 ── */}
              <h3 className="tsb-group__title">{dict.torrents2.groupBasic}</h3>
              <div className="tsb-grid">
                {/* 分类（多选 chip） */}
                <section className="tsb-card tsb-card--wide">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.catLegend}</h2>
                    {selectedCats.size > 0 && (
                      <a
                        className="tsb-card__action"
                        href={withParam(sp, "category_id", undefined)}
                      >
                        {dict.torrents2.catClear}
                      </a>
                    )}
                  </header>
                  <div className="tsb-chips tsb-chips--scroll">
                    {categories.map((c) =>
                      chipLabel("category_id", String(c.id), c.label, selectedCats.has(c.id)),
                    )}
                  </div>
                </section>

                {/* 存活（三态） */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.aliveLegend}</h2>
                  </header>
                  <div className="tsb-chips">
                    {(
                      [
                        ["0", dict.torrents2.aliveAll],
                        ["1", dict.torrents2.aliveAliveOnly],
                        ["2", dict.torrents2.aliveDead],
                      ] as [string, string][]
                    ).map(([v, label]) =>
                      chipLabel("alive", v, label, (sp.alive ?? "0") === v, "radio"),
                    )}
                  </div>
                </section>

                {/* 审核状态 */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.approvalLegend}</h2>
                  </header>
                  <div className="tsb-chips">
                    {(
                      [
                        ["", dict.torrents2.approvalAll],
                        ["1", dict.torrents2.approvalPassed],
                        ["2", dict.torrents2.approvalRejected],
                      ] as [string, string][]
                    ).map(([v, label]) =>
                      chipLabel("approval", v, label, (sp.approval ?? "") === v, "radio"),
                    )}
                  </div>
                </section>

                {/* 优惠 */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.promoLegend}</h2>
                  </header>
                  <div className="tsb-chips">
                    {(
                      [
                        ["", dict.torrents2.promoAny],
                        ["free", dict.torrents2.promoFree],
                        ["x2", dict.torrents2.promoX2],
                        ["half", dict.torrents2.promoHalf],
                        ["none", dict.torrents2.promoNone],
                      ] as [string, string][]
                    ).map(([v, label]) =>
                      chipLabel("promo", v, label, (sp.promo ?? "") === v, "radio"),
                    )}
                  </div>
                </section>
              </div>
            </div>

            <div className="tsb-group">
              {/* ── 数值与时间 ── */}
              <h3 className="tsb-group__title">{dict.torrents2.groupMetric}</h3>
              <div className="tsb-grid">
                {/* 发布时间区间（纵排，避免日期控件并排被截断） */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.dateLegend}</h2>
                  </header>
                  {rangeInputs(
                    dict.torrents2.dateLegend,
                    "date",
                    dict.torrents2.dateFrom,
                    dict.torrents2.dateTo,
                    sp.date_from,
                    sp.date_to,
                    true,
                  )}
                </section>

                {/* 体积区间（带单位文本） */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.sizeLegend}</h2>
                  </header>
                  {rangeInputs(
                    dict.torrents2.sizeLegend,
                    "size",
                    dict.torrents2.sizeFromPh,
                    dict.torrents2.sizeToPh,
                    sp.size_min,
                    sp.size_max,
                  )}
                  <p className="tsb-hint">{dict.torrents2.sizeHint}</p>
                </section>

                {/* 做种数区间 */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.seedersLegend}</h2>
                  </header>
                  {rangeInputs(
                    dict.torrents2.seedersLegend,
                    "min_seeders",
                    dict.torrents2.seedersFromPh,
                    dict.torrents2.seedersToPh,
                    sp.min_seeders,
                    undefined,
                  )}
                </section>

                {/* 下载数区间（0118 补齐） */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.leechersLegend}</h2>
                  </header>
                  {rangeInputs(
                    dict.torrents2.leechersLegend,
                    "leechers",
                    dict.torrents2.seedersFromPh,
                    dict.torrents2.seedersToPh,
                    sp.min_leechers,
                    sp.max_leechers,
                  )}
                </section>

                {/* 完成数区间（0118 补齐） */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.completedLegend}</h2>
                  </header>
                  {rangeInputs(
                    dict.torrents2.completedLegend,
                    "completed",
                    dict.torrents2.seedersFromPh,
                    dict.torrents2.seedersToPh,
                    sp.min_completed,
                    sp.max_completed,
                  )}
                </section>
              </div>
            </div>

            <div className="tsb-group">
              {/* ── 发布者与状态 ── */}
              <h3 className="tsb-group__title">{dict.torrents2.groupOwner}</h3>
              <div className="tsb-grid">
                {/* 发布者 / 匿名 */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.ownerLegend}</h2>
                  </header>
                  <div className="tsb-stack">
                    <input
                      className="tsb-input tsb-input--slim"
                      name="owner"
                      type="text"
                      defaultValue={sp.owner}
                      placeholder={dict.torrents2.ownerPh}
                      autoComplete="off"
                      aria-label={dict.torrents2.ownerLegend}
                    />
                    <div className="tsb-chips">
                      {(
                        [
                          ["0", dict.torrents2.anonAny],
                          ["1", dict.torrents2.anonOnly],
                          ["2", dict.torrents2.anonNamed],
                        ] as [string, string][]
                      ).map(([v, label]) =>
                        chipLabel("anonymous", v, label, (sp.anonymous ?? "0") === v, "radio"),
                      )}
                    </div>
                  </div>
                </section>

                {/* 种子状态（我在该种的做种/下载状态） */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.statusLegend}</h2>
                  </header>
                  <div className="tsb-chips">
                    {(
                      [
                        ["", dict.torrents2.statusAll],
                        ["seeding", dict.torrents2.stSeeding],
                        ["leeching", dict.torrents2.stLeeching],
                        ["completed", dict.torrents2.stCompleted],
                        ["incomplete", dict.torrents2.stIncomplete],
                        ["notseeding", dict.torrents2.stNotSeeding],
                      ] as [string, string][]
                    ).map(([v, label]) =>
                      chipLabel("status", v, label, (sp.status ?? "") === v, "radio"),
                    )}
                  </div>
                </section>

                {/* 官种 / 仅我 */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.otherLegend}</h2>
                  </header>
                  <div className="tsb-chips">
                    {chipLabel("official", "1", dict.torrents2.officialOnly, sp.official === "1")}
                    {chipLabel("mine", "1", dict.torrents2.mineOnly, sp.mine === "1")}
                  </div>
                </section>

                {/* 排除关键字 */}
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.excludeLegend}</h2>
                  </header>
                  <input
                    className="tsb-input tsb-input--slim"
                    name="exclude"
                    type="text"
                    defaultValue={sp.exclude}
                    placeholder={dict.torrents2.excludePh}
                    autoComplete="off"
                    aria-label={dict.torrents2.excludeLegend}
                  />
                </section>
              </div>
            </div>

            {/* ── 标签与排序 ── */}
            <div className="tsb-group">
              <h3 className="tsb-group__title">{dict.torrents2.groupOrder}</h3>
              <div className="tsb-grid tsb-grid--half">
                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.tagLabel}</h2>
                  </header>
                  <label className="tsb-row">
                    <span className="tsb-row__label">{dict.torrents2.tagLabel}</span>
                    <select name="tag_id" defaultValue={sp.tag_id ?? ""} className="tsb-select">
                      <option value="">{dict.torrents2.tagAny}</option>
                      {tags
                        .filter((t) => t.kind !== "official")
                        .map((t) => (
                          <option key={t.id} value={t.id}>{t.name}</option>
                        ))}
                    </select>
                  </label>
                </section>

                <section className="tsb-card">
                  <header className="tsb-card__head">
                    <h2 className="tsb-card__title">{dict.torrents2.sort}</h2>
                  </header>
                  <label className="tsb-row">
                    <span className="tsb-row__label">{dict.torrents2.sort}</span>
                    <select name="sort" defaultValue={sp.sort ?? ""} className="tsb-select">
                      <option value="">{dict.torrents2.sortDefault}</option>
                      <option value="seeders">{dict.torrents2.sortSeeders}</option>
                      <option value="leechers">{dict.torrents2.sortLeechers}</option>
                      <option value="size">{dict.torrents2.sortSize}</option>
                      <option value="completed">{dict.torrents2.sortCompleted}</option>
                      <option value="created">{dict.torrents2.sortCreatedDesc}</option>
                      <option value="created_asc">{dict.torrents2.sortCreatedAsc}</option>
                      <option value="name_asc">{dict.torrents2.sortName}</option>
                    </select>
                  </label>
                </section>
              </div>
            </div>

            {/* ── 多维筛选（站方可自建维度；同维度多选取 OR，跨维度取 AND） ── */}
            {dimKinds.length > 0 && (
              <div className="tsb-group">
                <h3 className="tsb-group__title">{dict.torrents2.dimLegend}</h3>
                <div className="tsb-stack">
                  {dimKinds.map((k) => (
                    <div key={k.kind} className="tsb-dim">
                      <span className="tsb-dim__label">{k.label}</span>
                      <div className="tsb-chips">
                        {(secDict?.[k.kind] ?? []).map((d) =>
                          chipLabel(
                            `sec_${k.kind}`,
                            String(d.id),
                            d.name,
                            (sp[`sec_${k.kind}`] ?? "").split(",").includes(String(d.id)),
                          ),
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* 面板操作条 */}
            <div className="tsb-actions">
              <a href="/torrents" className="tsb-reset">{dict.torrents2.advReset}</a>
              <button type="submit" className="baozi-button">{dict.torrents2.advApply}</button>
            </div>
          </div>
        </details>
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
                  <a href={withParam(sp, "sort", toggleSort(sp.sort, "name"))}>{dict.torrents.colTitle}</a>
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
                  <a href={withParam(sp, "sort", toggleSort(sp.sort, "leechers"))}>⬇️</a>
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
