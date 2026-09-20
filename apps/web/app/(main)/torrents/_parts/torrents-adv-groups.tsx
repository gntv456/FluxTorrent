/**
 * 种子列表页高级搜索·上半部分组（从 app/(main)/torrents/page.tsx 按域拆出）：
 * 基础筛选 / 数值与时间 两组卡片 + 选项 chip / 数值区间渲染小件。
 * 下半部（发布者与状态 / 标签与排序 / 多维筛选）在 torrents-adv-groups-tail.tsx。
 */

import type { Dict } from "@/i18n/zh-CN";
import type { SectionDictRow, SectionKindMeta, TorrentsSP } from "./torrents-utils";

/** 高级分组共享的渲染上下文（由搜索盒传入） */
export interface AdvGroupsCtx {
  dict: Dict;
  sp: TorrentsSP;
  categories: { id: number; label: string }[];
  tags: { id: number; name: string; kind: string }[];
  secDict: Record<string, SectionDictRow[]> | null;
  dimKinds: SectionKindMeta[];
  selectedCats: Set<number>;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
}

/** 选项 chips（checkbox 多选 / radio 单选共用渲染） */
export function chipLabel(
  name: string,
  value: string,
  text: string,
  checked: boolean,
  type: "checkbox" | "radio" = "checkbox",
) {
  return (
    <label key={`${name}-${value}`} className="tsb-chip">
      <input type={type} name={name} value={value} defaultChecked={checked} />
      <span>{text}</span>
    </label>
  );
}

/** 行内数值区间（min/max 一对输入；name 前缀 + legend 拼装 aria） */
export function rangeInputs(
  legend: string,
  name: string,
  minPh: string,
  maxPh: string,
  minVal?: string,
  maxVal?: string,
  date = false,
) {
  return (
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
}

export function TorrentsAdvGroups(ctx: AdvGroupsCtx) {
  const { dict, sp, categories, selectedCats, withParam } = ctx;
  const t2 = dict.torrents2;
  return (
    <>
      <div className="tsb-group">
        {/* ── 基础筛选 ── */}
        <h3 className="tsb-group__title">{t2.groupBasic}</h3>
        <div className="tsb-grid">
          {/* 分类（多选 chip） */}
          <section className="tsb-card tsb-card--wide">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.catLegend}</h2>
              {selectedCats.size > 0 && (
                <a
                  className="tsb-card__action"
                  href={withParam(sp, "category_id", undefined)}
                >
                  {t2.catClear}
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
              <h2 className="tsb-card__title">{t2.aliveLegend}</h2>
            </header>
            <div className="tsb-chips">
              {(
                [
                  ["0", t2.aliveAll],
                  ["1", t2.aliveAliveOnly],
                  ["2", t2.aliveDead],
                ] as [string, string][]
              ).map(([v, label]) =>
                chipLabel("alive", v, label, (sp.alive ?? "0") === v, "radio"),
              )}
            </div>
          </section>

          {/* 审核状态 */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.approvalLegend}</h2>
            </header>
            <div className="tsb-chips">
              {(
                [
                  ["", t2.approvalAll],
                  ["1", t2.approvalPassed],
                  ["2", t2.approvalRejected],
                ] as [string, string][]
              ).map(([v, label]) =>
                chipLabel("approval", v, label, (sp.approval ?? "") === v, "radio"),
              )}
            </div>
          </section>

          {/* 优惠 */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.promoLegend}</h2>
            </header>
            <div className="tsb-chips">
              {(
                [
                  ["", t2.promoAny],
                  ["free", t2.promoFree],
                  ["x2", t2.promoX2],
                  ["half", t2.promoHalf],
                  ["none", t2.promoNone],
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
        <h3 className="tsb-group__title">{t2.groupMetric}</h3>
        <div className="tsb-grid">
          {/* 发布时间区间（纵排，避免日期控件并排被截断） */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.dateLegend}</h2>
            </header>
            {rangeInputs(
              t2.dateLegend,
              "date",
              t2.dateFrom,
              t2.dateTo,
              sp.date_from,
              sp.date_to,
              true,
            )}
          </section>

          {/* 体积区间（带单位文本） */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.sizeLegend}</h2>
            </header>
            {rangeInputs(
              t2.sizeLegend,
              "size",
              t2.sizeFromPh,
              t2.sizeToPh,
              sp.size_min,
              sp.size_max,
            )}
            <p className="tsb-hint">{t2.sizeHint}</p>
          </section>

          {/* 做种数区间 */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.seedersLegend}</h2>
            </header>
            {rangeInputs(
              t2.seedersLegend,
              "min_seeders",
              t2.seedersFromPh,
              t2.seedersToPh,
              sp.min_seeders,
              undefined,
            )}
          </section>

          {/* 下载数区间（0118 补齐） */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.leechersLegend}</h2>
            </header>
            {rangeInputs(
              t2.leechersLegend,
              "leechers",
              t2.seedersFromPh,
              t2.seedersToPh,
              sp.min_leechers,
              sp.max_leechers,
            )}
          </section>

          {/* 完成数区间（0118 补齐） */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.completedLegend}</h2>
            </header>
            {rangeInputs(
              t2.completedLegend,
              "completed",
              t2.seedersFromPh,
              t2.seedersToPh,
              sp.min_completed,
              sp.max_completed,
            )}
          </section>
        </div>
      </div>
    </>
  );
}
