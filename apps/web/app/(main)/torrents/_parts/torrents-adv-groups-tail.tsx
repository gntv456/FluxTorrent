/**
 * 种子列表页高级搜索·下半部分组（从 _parts/torrents-adv-groups.tsx 按域再拆）：
 * 发布者与状态 / 标签与排序 / 多维筛选 三组卡片。上半部（基础筛选 / 数值与时间）
 * 留在 torrents-adv-groups.tsx。
 */

import {
  chipLabel as ctxAdvChip,
  type AdvGroupsCtx,
} from "./torrents-adv-groups";

export function TorrentsAdvGroupsTail(ctx: AdvGroupsCtx) {
  const { dict, sp, tags, tagGroups, secDict, dimKinds } = ctx;
  const t2 = dict.torrents2;
  return (
    <>
      <div className="tsb-group">
        {/* ── 发布者与状态 ── */}
        <h3 className="tsb-group__title">{t2.groupOwner}</h3>
        <div className="tsb-grid">
          {/* 发布者 / 匿名 */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.ownerLegend}</h2>
            </header>
            <div className="tsb-stack">
              <input
                className="tsb-input tsb-input--slim"
                name="owner"
                type="text"
                defaultValue={sp.owner}
                placeholder={t2.ownerPh}
                autoComplete="off"
                aria-label={t2.ownerLegend}
              />
              <div className="tsb-chips">
                {(
                  [
                    ["0", t2.anonAny],
                    ["1", t2.anonOnly],
                    ["2", t2.anonNamed],
                  ] as [string, string][]
                ).map(([v, label]) =>
                  ctxAdvChip(
                    "anonymous",
                    v,
                    label,
                    (sp.anonymous ?? "0") === v,
                    "radio",
                  ),
                )}
              </div>
            </div>
          </section>

          {/* 种子状态（我在该种的做种/下载状态） */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.statusLegend}</h2>
            </header>
            <div className="tsb-chips">
              {(
                [
                  ["", t2.statusAll],
                  ["seeding", t2.stSeeding],
                  ["leeching", t2.stLeeching],
                  ["completed", t2.stCompleted],
                  ["incomplete", t2.stIncomplete],
                  ["notseeding", t2.stNotSeeding],
                ] as [string, string][]
              ).map(([v, label]) =>
                ctxAdvChip(
                  "status",
                  v,
                  label,
                  (sp.status ?? "") === v,
                  "radio",
                ),
              )}
            </div>
          </section>

          {/* 官种 / 仅我 / 书签（阶段三筛选粒度补书签） */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.otherLegend}</h2>
            </header>
            <div className="tsb-chips">
              {ctxAdvChip(
                "official",
                "1",
                t2.officialOnly,
                sp.official === "1",
              )}
              {ctxAdvChip("mine", "1", t2.mineOnly, sp.mine === "1")}
              {ctxAdvChip(
                "bookmarked",
                "1",
                t2.bookmarkedOnly,
                sp.bookmarked === "1",
              )}
            </div>
          </section>

          {/* 排除关键字 */}
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.excludeLegend}</h2>
            </header>
            <input
              className="tsb-input tsb-input--slim"
              name="exclude"
              type="text"
              defaultValue={sp.exclude}
              placeholder={t2.excludePh}
              autoComplete="off"
              aria-label={t2.excludeLegend}
            />
          </section>
        </div>
      </div>

      {/* ── 标签与排序 ── */}
      <div className="tsb-group">
        <h3 className="tsb-group__title">{t2.groupOrder}</h3>
        <div className="tsb-grid tsb-grid--half">
          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.tagLabel}</h2>
            </header>
            {/* 0159 P1：标签多选 chips + any/all 匹配模式（与 promo 多选同交互）；
                0160 P2：按组分区（属性/内容），无组信息退化平铺 */}
            {(["attribute", "content"] as const)
              .filter((g) =>
                tags.some(
                  (t, i) =>
                    t.kind !== "official" &&
                    (tagGroups?.[i] ?? "attribute") === g,
                ),
              )
              .map((g) => (
                <div
                  key={g}
                  className="tsb-chips tsb-chips--scroll"
                  data-tag-group={g}
                >
                  {(tagGroups ?? []).some((x) => x === "content") && (
                    <span className="tsb-chip__grouplabel">
                      {g === "attribute"
                        ? (t2.tagGroupAttr ?? "属性")
                        : (t2.tagGroupContent ?? "内容")}
                    </span>
                  )}
                  {tags
                    .filter(
                      (t, i) =>
                        t.kind !== "official" &&
                        (tagGroups?.[i] ?? "attribute") === g,
                    )
                    .map((t) =>
                      ctxAdvChip(
                        "tag_ids",
                        String(t.id),
                        t.name,
                        (sp.tag_ids ?? sp.tag_id ?? "")
                          .split(",")
                          .includes(String(t.id)),
                      ),
                    )}
                </div>
              ))}
            <div className="tsb-chips">
              {(
                [
                  ["any", t2.tagMatchAny],
                  ["all", t2.tagMatchAll],
                ] as [string, string][]
              ).map(([v, label]) =>
                ctxAdvChip(
                  "tag_mode",
                  v,
                  label,
                  (sp.tag_mode ?? "any") === v,
                  "radio",
                ),
              )}
            </div>
            <p className="tsb-hint">{t2.tagMultiHint}</p>
          </section>

          <section className="tsb-card">
            <header className="tsb-card__head">
              <h2 className="tsb-card__title">{t2.sort}</h2>
            </header>
            <label className="tsb-row">
              <span className="tsb-row__label">{t2.sort}</span>
              <select
                name="sort"
                defaultValue={sp.sort ?? ""}
                className="tsb-select"
              >
                <option value="">{t2.sortDefault}</option>
                <option value="seeders">{t2.sortSeeders}</option>
                <option value="leechers">{t2.sortLeechers}</option>
                <option value="size">{t2.sortSize}</option>
                <option value="completed">{t2.sortCompleted}</option>
                <option value="created">{t2.sortCreatedDesc}</option>
                <option value="created_asc">{t2.sortCreatedAsc}</option>
                <option value="name_asc">{t2.sortName}</option>
              </select>
            </label>
          </section>
        </div>
      </div>

      {/* ── 多维筛选（站方可自建维度；同维度多选取 OR，跨维度取 AND） ── */}
      {dimKinds.length > 0 && (
        <div className="tsb-group">
          <h3 className="tsb-group__title">{t2.dimLegend}</h3>
          <div className="tsb-stack">
            {dimKinds.map((k) => (
              <div key={k.kind} className="tsb-dim">
                <span className="tsb-dim__label">{k.label}</span>
                <div className="tsb-chips">
                  {(secDict?.[k.kind] ?? []).map((d) =>
                    ctxAdvChip(
                      `sec_${k.kind}`,
                      String(d.id),
                      d.name,
                      (sp[`sec_${k.kind}`] ?? "")
                        .split(",")
                        .includes(String(d.id)),
                    ),
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
