"use client";

import { useMemo, useState } from "react";
import type { Medal } from "@fluxtorrent/domain-types";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { MedalRarity } from "@/lib/medal-rarity";
import { MedalCard } from "@/components/medal-card";
import { useIsCompact } from "@/lib/hooks/use-media";

// 勋章殿堂主体（客户端）：分类 Tab + 筛选 + 排序都是本地筛选，接口不动。
// 卡片渲染在 SSR 阶段也会跑，故「售卖截止」用挂载无关的固定时区格式化 +
// suppressHydrationWarning（容器 UTC vs 浏览器 GMT+8，见仓库 hydration 纪律）。
//
// 2026-11-02 手机端：单卡原本摊开六行规格表（≈440px），二十枚就是十屏。
// 窄屏改为「一行摘要 + 点详情才展开」（见 medal-card.tsx），并且先出八张、
// 底部给「展开全部」——桌面端不参与这两处，行为与改前一致。

/** 按稀有度排序权重（词表顺序即档位高低；词表没有的排最后） */
function rankOf(list: MedalRarity[], v?: string | null): number {
  const i = list.findIndex((r) => r.value === v);
  return i < 0 ? list.length : i;
}

/** 窄屏首屏先出几张（其余点「展开全部」再看） */
const MOBILE_FIRST = 8;

export function MedalHall({
  medals,
  rarities,
  maxWorn,
  globalGiftTaxBp,
  balance,
}: {
  medals: Medal[];
  rarities: MedalRarity[];
  /** 佩戴上限（0204；站点设置下发） */
  maxWorn?: number;
  /** 全站赠送税率（基点；仅用于赠送按钮费率提示） */
  globalGiftTaxBp?: number;
  /** 当前用户魔力余额（null=匿名/未知：不做余额预判，购买交后端校验） */
  balance?: number | null;
}) {
  const { dict, locale, currency } = useI18n();
  const t = dict.medals;
  const g = dict.medals2;
  const compact = useIsCompact();
  const [cat, setCat] = useState<number | null>(null);
  const [only, setOnly] = useState("all");
  const [sort, setSort] = useState("rarity");
  const [showAll, setShowAll] = useState(false);
  // 0204 多佩戴：佩戴计数本地维护（wear 回调触发重算，免整页拉取）
  const [wornCount, setWornCount] = useState(
    () => medals.filter((m) => m.wearing).length,
  );

  const cats = useMemo(() => {
    const m = new Map<number, string>();
    for (const x of medals) {
      m.set(x.category_id, x.category_name ?? g.uncategorized);
    }
    return [...m.entries()];
  }, [medals, g.uncategorized]);

  const list = useMemo(() => {
    let out = medals;
    if (cat !== null) out = out.filter((m) => m.category_id === cat);
    if (only === "buyable") {
      out = out.filter((m) => m.get_type === 1 && m.price !== null);
    }
    if (only === "owned") out = out.filter((m) => m.owned);
    const copy = [...out];
    if (sort === "rarity") {
      copy.sort(
        (a, b) =>
          rankOf(rarities, a.rarity) - rankOf(rarities, b.rarity) ||
          a.id - b.id,
      );
    } else if (sort === "price") {
      copy.sort((a, b) => (b.price ?? -1) - (a.price ?? -1));
    } else {
      copy.sort((a, b) => b.bonus_addition_factor - a.bonus_addition_factor);
    }
    return copy;
  }, [medals, cat, only, sort, rarities]);

  const nBuyable = medals.filter(
    (m) => m.get_type === 1 && m.price !== null,
  ).length;
  const nOwned = medals.filter((m) => m.owned).length;

  // 窄屏：先出八张；桌面：全出（与改前一致）
  const shown = compact && !showAll ? list.slice(0, MOBILE_FIRST) : list;

  return (
    <div className="flex flex-col gap-3">
      <div className="pgtools">
        <div className="tide-tabs" role="tablist">
          <button
            type="button"
            role="tab"
            className="tide-tab"
            aria-selected={cat === null}
            onClick={() => setCat(null)}
          >
            {t.groupAll} {medals.length}
          </button>
          {cats.map(([id, name]) => (
            <button
              key={id}
              role="tab"
              className="tide-tab"
              aria-selected={cat === id}
              onClick={() => setCat(id)}
            >
              {name} {medals.filter((m) => m.category_id === id).length}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="chip-sel"
          aria-pressed={only === "all"}
          onClick={() => setOnly("all")}
        >
          {t.filterAll}
        </button>
        <button
          type="button"
          className="chip-sel"
          aria-pressed={only === "buyable"}
          onClick={() => setOnly("buyable")}
        >
          {fmt(t.filterBuyable, { n: nBuyable })}
        </button>
        <button
          type="button"
          className="chip-sel"
          aria-pressed={only === "owned"}
          onClick={() => setOnly("owned")}
        >
          {fmt(t.filterOwned, { n: nOwned })}
        </button>
        <select
          className="pgsel ml-auto"
          value={sort}
          onChange={(e) => setSort(e.target.value)}
          aria-label={t.sortLabel}
        >
          <option value="rarity">{t.sortRarity}</option>
          <option value="price">{t.sortPrice}</option>
          <option value="bonus">{t.sortBonus}</option>
        </select>
        {maxWorn != null && (
          <span className="text-xs text-sub whitespace-nowrap">
            {fmt(t.wornCount, { n: wornCount, max: maxWorn })}
          </span>
        )}
      </div>

      {list.length === 0 && (
        <p className="brand-panel p-4 text-sm text-sub">{t.emptyFilter}</p>
      )}

      <div className="hall-grid">
        {shown.map((m) => (
          <MedalCard
            key={m.id}
            m={m}
            rarities={rarities}
            t={t}
            g={g}
            locale={locale}
            currency={currency}
            balance={balance}
            globalGiftTaxBp={globalGiftTaxBp}
            onWearChange={() =>
              setWornCount(
                medals.filter((x) => x.wearing).length +
                  (m.wearing ? -1 : 1),
              )
            }
          />
        ))}
      </div>

      {compact && !showAll && list.length > MOBILE_FIRST && (
        <button
          type="button"
          className="brand-button self-center"
          onClick={() => setShowAll(true)}
        >
          {t.showAllMedals.replace("{n}", String(list.length))}
        </button>
      )}
    </div>
  );
}
