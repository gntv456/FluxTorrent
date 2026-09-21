/**
 * 种子列表页已选条件摘要（从 app/(main)/torrents/page.tsx 按域拆出）：
 * buildTorrentChips 依据当前查询参数拼装可移除 chip 列表（含多维筛选）。
 */

import type { Dict } from "@/i18n/zh-CN";
import type {
  SectionDictRow,
  SectionKindMeta,
  TorrentsSP,
} from "./torrents-utils";

export interface TorrentChip {
  key: string;
  text: string;
  href: string;
}

export function buildTorrentChips(opts: {
  sp: TorrentsSP;
  dict: Dict;
  categories: { id: number; label: string }[];
  tags: { id: number; name: string; kind: string }[];
  secDict: Record<string, SectionDictRow[]> | null;
  dimKinds: SectionKindMeta[];
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
}): TorrentChip[] {
  const { sp, dict, categories, tags, secDict, dimKinds, withParam } = opts;
  const t2 = dict.torrents2;
  const chips: TorrentChip[] = [];
  const chip = (key: string, text: string) => {
    chips.push({ key, text, href: withParam(sp, key, undefined) });
  };
  const areaLabels: Record<string, string> = {
    "0": t2.areaTitle,
    "1": t2.areaDescr,
    "3": t2.areaUploader,
    "4": "IMDb",
  };
  if (sp.search) chip("search", `${t2.keywordPh}：${sp.search}`);
  if (sp.search_area && sp.search_area !== "0") {
    chip(
      "search_area",
      `${t2.scope}：${areaLabels[sp.search_area] ?? sp.search_area}`,
    );
  }
  if (sp.search_mode === "2") chip("search_mode", t2.modeExact);
  if (sp.category_id) {
    const catName = (id: string) =>
      categories.find((c) => String(c.id) === id)?.label ?? id;
    const names = sp.category_id.split(",").filter(Boolean).map(catName);
    chip(
      "category_id",
      `${t2.catLegend}：${names.slice(0, 3).join("、")}${names.length > 3 ? ` +${names.length - 3}` : ""}`,
    );
  }
  if (sp.alive === "1") chip("alive", t2.aliveAliveOnly);
  if (sp.alive === "2") chip("alive", t2.aliveDead);
  if (sp.status) {
    const stLabels: Record<string, string> = {
      seeding: t2.stSeeding,
      leeching: t2.stLeeching,
      completed: t2.stCompleted,
      incomplete: t2.stIncomplete,
      notseeding: t2.stNotSeeding,
    };
    chip("status", `${t2.statusLegend}：${stLabels[sp.status] ?? sp.status}`);
  }
  if (sp.approval === "1")
    chip("approval", `${t2.approvalLegend}：${t2.approvalPassed}`);
  if (sp.approval === "2")
    chip("approval", `${t2.approvalLegend}：${t2.approvalRejected}`);
  if (sp.official === "1") chip("official", t2.officialOnly);
  if (sp.mine === "1") chip("mine", t2.mineOnly);
  if (sp.tag_id) {
    const t = tags.find((x) => String(x.id) === sp.tag_id);
    chip("tag_id", `${t2.tagLabel}：${t?.name ?? sp.tag_id}`);
  }
  if (sp.promo) {
    const prLabels: Record<string, string> = {
      free: t2.promoFree,
      x2: t2.promoX2,
      half: t2.promoHalf,
      any: t2.promoHas,
      none: t2.promoNone,
    };
    chip("promo", `${t2.promoLegend}：${prLabels[sp.promo] ?? sp.promo}`);
  }
  if (sp.size_min) chip("size_min", `${t2.sizeLegend} ≥ ${sp.size_min}`);
  if (sp.size_max) chip("size_max", `${t2.sizeLegend} ≤ ${sp.size_max}`);
  if (sp.min_seeders)
    chip("min_seeders", `${t2.seedersLegend} ≥ ${sp.min_seeders}`);
  if (sp.max_seeders)
    chip("max_seeders", `${t2.seedersLegend} ≤ ${sp.max_seeders}`);
  if (sp.min_leechers)
    chip("min_leechers", `${t2.leechersLegend} ≥ ${sp.min_leechers}`);
  if (sp.max_leechers)
    chip("max_leechers", `${t2.leechersLegend} ≤ ${sp.max_leechers}`);
  if (sp.min_completed)
    chip("min_completed", `${t2.completedLegend} ≥ ${sp.min_completed}`);
  if (sp.max_completed)
    chip("max_completed", `${t2.completedLegend} ≤ ${sp.max_completed}`);
  if (sp.date_from) chip("date_from", `${t2.dateLegend} ≥ ${sp.date_from}`);
  if (sp.date_to) chip("date_to", `${t2.dateLegend} ≤ ${sp.date_to}`);
  if (sp.owner) chip("owner", `${t2.ownerLegend}：${sp.owner}`);
  if (sp.exclude) chip("exclude", `${t2.excludeLegend}：${sp.exclude}`);
  if (sp.anonymous === "1") chip("anonymous", t2.anonOnly);
  if (sp.anonymous === "2") chip("anonymous", t2.anonNamed);
  for (const k of dimKinds) {
    const raw = sp[`sec_${k.kind}`];
    if (!raw) continue;
    const names = raw
      .split(",")
      .filter(Boolean)
      .map(
        (id) =>
          (secDict?.[k.kind] ?? []).find((d) => String(d.id) === id)?.name ??
          id,
      );
    chip(`sec_${k.kind}`, `${k.label}：${names.join("、")}`);
  }
  return chips;
}
