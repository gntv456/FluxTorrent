/**
 * 资源库分面发现（A2）：搜索行下方的分类 / 分辨率 / 体积 / 更多 四组分面。
 * 纯 server component：所有状态走 URL，chips 即链接，天然可分享、可后退；
 * 只有「保存筛选」需要一个客户端小件（见 ./torrents-facet-save）。
 *
 * 后端没有固定的 resolution / subtitle 字段：这两组只在站点的现有维度字典
 * 或标签字典里找到对应项时才出现，一律不造硬编码 id 的无效链接。
 */

import type { Dict } from "@/i18n/zh-CN";
import type {
  SectionDictRow,
  SectionKindMeta,
  TorrentsSP,
} from "./torrents-utils";
import { withoutCursor, withParam } from "./torrents-utils";
import { FacetSaveButton } from "./torrents-facet-save";

/** 一次改多个参数：借 withParam 的序列化口径（哨兵键不存在 = 只序列化），
 *  与全站链接生成同规则，避免分面自己拼查询串。 */
function facetHref(
  sp: TorrentsSP,
  entries: Record<string, string | undefined>,
): string {
  return withParam(
    { ...withoutCursor(sp), ...entries },
    "__facet_noop__",
    undefined,
  );
}

function inCsv(cur: string | undefined, value: string): boolean {
  return (cur ?? "").split(",").some((x) => x.trim() === value);
}

/** 逗号串多选 toggle：保留其余已选值，清空则整参删除 */
function toggleCsv(cur: string | undefined, value: string): string | undefined {
  const next = (cur ?? "")
    .split(",")
    .map((x) => x.trim())
    .filter(Boolean);
  const i = next.indexOf(value);
  if (i >= 0) next.splice(i, 1);
  else next.push(value);
  return next.length ? next.join(",") : undefined;
}

function FacetChip({
  href,
  label,
  active,
}: {
  href: string;
  label: string;
  active: boolean;
}) {
  return (
    <a href={href} className="ux-facets__chip" aria-pressed={active}>
      {label}
    </a>
  );
}

function FacetRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="ux-facets__row">
      <span className="ux-facets__label">{label}</span>
      <div className="ux-facets__chips">{children}</div>
    </div>
  );
}

/** 分辨率目标：只在站点分辨率维度里找到同名/近名选项时才给 chip */
const RESOLUTION_TARGETS: readonly [string, RegExp][] = [
  ["4K", /(^|[^a-z0-9])4k|2160/i],
  ["1080p", /1080/i],
  ["720p", /720/i],
];
/** 中文字幕：标签或维度选项命中才给 chip（避免匹配「字幕组」发布组标签） */
const SUBTITLE_RE = /中文字幕|chinese[-_ ]?sub|zh[-_ ]?sub/i;

export function TorrentsFacets({
  dict,
  sp,
  categories,
  tags,
  secDict,
  dimKinds,
}: {
  dict: Dict;
  sp: TorrentsSP;
  /** 已由 page.tsx 从同一次 site-profile 请求取出的顶级分类 */
  categories: { id: number; label: string }[];
  tags: { id: number; name: string; kind: string }[];
  secDict: Record<string, SectionDictRow[]> | null;
  dimKinds: SectionKindMeta[];
}) {
  const u = dict.uxOpt;
  const t2 = dict.torrents2;

  // 分辨率：维度字典里 kind=resolution 的选项 → sec_resolution=<option.id>
  const resKind = dimKinds.find((k) => k.kind === "resolution");
  const resOptions = resKind ? (secDict?.[resKind.kind] ?? []) : [];
  const resolutions = RESOLUTION_TARGETS.flatMap(([, re]) => {
    const opt = resOptions.find((o) => re.test(o.name));
    return opt
      ? [
          {
            key: "sec_" + resKind!.kind,
            value: String(opt.id),
            label: opt.name,
          },
        ]
      : [];
  });

  // 中文字幕：优先标签（tag_ids），其次任意非分辨率维度的同名选项
  let subtitle: { key: string; value: string } | null = null;
  const subTag = tags.find(
    (t) => t.kind !== "official" && SUBTITLE_RE.test(t.name),
  );
  if (subTag) {
    subtitle = { key: "tag_ids", value: String(subTag.id) };
  } else {
    for (const k of dimKinds) {
      if (k.kind === "resolution") continue;
      const opt = (secDict?.[k.kind] ?? []).find((o) =>
        SUBTITLE_RE.test(o.name),
      );
      if (opt) {
        subtitle = { key: "sec_" + k.kind, value: String(opt.id) };
        break;
      }
    }
  }
  const subActive = subtitle
    ? inCsv(
        subtitle.key === "tag_ids" ? sp.tag_ids ?? sp.tag_id : sp[subtitle.key],
        subtitle.value,
      )
    : false;

  const sizeChips: { label: string; min?: string; max?: string }[] = [
    { label: "<1GB", max: "1GB" },
    { label: "1–10GB", min: "1GB", max: "10GB" },
    { label: ">10GB", min: "10GB" },
  ];

  // 默认折叠（站长拍板 2026-11-23）：四行 chip 墙占首屏过高，收进 details
  // 一行 summary；summary 常显「当前 N 项筛选」徽标，有筛选时折叠也不丢信息。
  const activeFacets =
    (sp.category_id ? 1 : 0) +
    (sp.promo ? 1 : 0) +
    (sp.size_min || sp.size_max ? 1 : 0) +
    (sp.alive === "1" ? 1 : 0) +
    (sp.official === "1" ? 1 : 0) +
    resolutions.filter((r) => inCsv(sp[r.key], r.value)).length +
    (subtitle && subActive ? 1 : 0);

  return (
    <details className="ux-facets-wrap">
      <summary className="ux-facets-summary">
        <span>{u.facetsToggle}</span>
        {activeFacets > 0 && (
          <span className="ux-facets-count" aria-label={u.filterSaved}>
            {activeFacets}
          </span>
        )}
      </summary>
      <div className="ux-facets">
      <FacetRow label={u.filterCategory}>
        {categories.map((c) => {
          const id = String(c.id);
          const active = inCsv(sp.category_id, id);
          return (
            <FacetChip
              key={id}
              href={facetHref(sp, {
                category_id: toggleCsv(sp.category_id, id),
              })}
              label={c.label}
              active={active}
            />
          );
        })}
      </FacetRow>

      {resolutions.length > 0 && (
        <FacetRow label={u.filterResolution}>
          {resolutions.map((r) => (
            <FacetChip
              key={r.value}
              href={facetHref(sp, {
                [r.key]: toggleCsv(sp[r.key], r.value),
              })}
              label={r.label}
              active={inCsv(sp[r.key], r.value)}
            />
          ))}
        </FacetRow>
      )}

      <FacetRow label={u.filterSize}>
        {sizeChips.map((s) => (
          <FacetChip
            key={s.label}
            href={facetHref(sp, {
              size_min: s.min === sp.size_min ? undefined : s.min,
              size_max: s.max === sp.size_max ? undefined : s.max,
            })}
            label={s.label}
            active={sp.size_min === s.min && sp.size_max === s.max}
          />
        ))}
      </FacetRow>

      {/* PM-2：Free/促销从二级高级面板提到首屏——新手生存第一条建议就靠它。
          后端 promo 支持多选，这里用单选 toggle（点已选项即清除），
          高级面板的多选组合仍可通过 URL 直达，不被本组件改写。 */}
      <FacetRow label={t2.promoLegend}>
        {[
          ["free", t2.promoFree],
          ["x2", t2.promoX2],
          ["half", t2.promoHalf],
          ["any", t2.promoHas],
          ["none", t2.promoNone],
        ].map(([v, label]) => (
          <FacetChip
            key={v}
            href={facetHref(sp, {
              promo: sp.promo === v ? undefined : v,
            })}
            label={label}
            active={sp.promo === v}
          />
        ))}
      </FacetRow>

      <FacetRow label={u.filterMore}>
        <FacetChip
          href={facetHref(sp, { alive: sp.alive === "1" ? undefined : "1" })}
          label={u.filterAlive}
          active={sp.alive === "1"}
        />
        <FacetChip
          href={facetHref(sp, {
            official: sp.official === "1" ? undefined : "1",
          })}
          label={u.filterOfficial}
          active={sp.official === "1"}
        />
        {subtitle && (
          <FacetChip
            href={facetHref(sp, {
              [subtitle.key]: toggleCsv(sp[subtitle.key], subtitle.value),
            })}
            label={u.filterSubtitled}
            active={subActive}
          />
        )}
      </FacetRow>

      <FacetSaveButton
        sp={sp}
        saveLabel={u.saveFilter}
        savedLabel={u.filterSaved}
      />
      </div>
    </details>
  );
}