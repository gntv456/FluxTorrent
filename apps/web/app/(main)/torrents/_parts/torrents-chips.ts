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
import { withoutValue } from "./torrents-utils";

export interface TorrentChip {
  /** 渲染用唯一键：多选参数会为同一 key 生成多个 chip，故不能直接用 key 当 React key */
  id: string;
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
    chips.push({ id: key, key, text, href: withParam(sp, key, undefined) });
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
    // 多选分类：每个已选分类各一个 chip，移除只去掉该分类（此前是整组清空）
    for (const id of sp.category_id.split(",").filter(Boolean)) {
      chips.push({
        id: `category_id:${id}`,
        key: "category_id",
        text: `${t2.catLegend}：${catName(id)}`,
        href: withoutValue(sp, "category_id", id),
      });
    }
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
  if (sp.bookmarked === "1") chip("bookmarked", t2.bookmarkedOnly);
  // 0159 P1：标签多选——每个已选标签各一个 chip，移除只去掉该标签；
  // 兼容旧单值 tag_id（拆入同逻辑）；all 模式追加一个模式 chip
  {
    const rawTag = sp.tag_ids ?? sp.tag_id ?? "";
    const tagName = (id: string) =>
      tags.find((x) => String(x.id) === id)?.name ?? `#${id}`;
    for (const id of rawTag.split(",").filter(Boolean)) {
      chips.push({
        id: `tag:${id}`,
        key: rawTag === sp.tag_id ? "tag_id" : "tag_ids",
        text: `${t2.tagLabel}：${tagName(id)}`,
        href: withoutValue(
          sp,
          rawTag === sp.tag_id ? "tag_id" : "tag_ids",
          id,
        ),
      });
    }
    if (rawTag && sp.tag_mode === "all") {
      chips.push({
        id: "tag_mode",
        key: "tag_mode",
        text: `${t2.tagLabel}：${t2.tagMatchAll}`,
        href: withParam(sp, "tag_mode", undefined),
      });
    }
  }
  if (sp.promo) {
    const prLabels: Record<string, string> = {
      free: t2.promoFree,
      x2: t2.promoX2,
      half: t2.promoHalf,
      any: t2.promoHas,
      none: t2.promoNone,
    };
    // 阶段三多选：free,x2 → 「免费/2倍」链式展示，点击 chip 移除整组
    const labelText = sp.promo
      .split(",")
      .map((v) => prLabels[v.trim()] ?? v.trim())
      .join("/");
    chip("promo", `${t2.promoLegend}：${labelText}`);
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
    const dimName = (id: string) =>
      (secDict?.[k.kind] ?? []).find((d) => String(d.id) === id)?.name ?? id;
    // 多维筛选：每个已选值各一个 chip，移除只去掉该值（此前是整组清空）
    for (const id of raw.split(",").filter(Boolean)) {
      chips.push({
        id: `sec_${k.kind}:${id}`,
        key: `sec_${k.kind}`,
        text: `${k.label}：${dimName(id)}`,
        href: withoutValue(sp, `sec_${k.kind}`, id),
      });
    }
  }
  return chips;
}
