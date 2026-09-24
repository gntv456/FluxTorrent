"use client";

import { useMemo, useState } from "react";
import type { Medal } from "@fluxtorrent/domain-types";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { medalRarityTone, type MedalRarity } from "@/lib/medal-rarity";
import { MedalBadge } from "@/components/medal-badge";
import { MedalRarityChip } from "@/components/medal-rarity-chip";
import { MedalActions } from "@/components/medal-actions";

// 勋章殿堂主体（客户端）：分类 Tab + 筛选 + 排序都是本地筛选，接口不动。
// 卡片渲染在 SSR 阶段也会跑，故「售卖截止」用挂载无关的固定时区格式化 +
// suppressHydrationWarning（容器 UTC vs 浏览器 GMT+8，见仓库 hydration 纪律）。

/** 固定 UTC 格式化：两端一致，避免 SSR/CSR 跨时区渲染出不同日期 */
function isoDay(iso: string, locale: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString(locale, { timeZone: "UTC" });
}

/** 按稀有度排序权重（词表顺序即档位高低；词表没有的排最后） */
function rankOf(list: MedalRarity[], v?: string | null): number {
  const i = list.findIndex((r) => r.value === v);
  return i < 0 ? list.length : i;
}

export function MedalHall({
  medals,
  rarities,
}: {
  medals: Medal[];
  rarities: MedalRarity[];
}) {
  const { dict, locale, currency } = useI18n();
  const t = dict.medals;
  const g = dict.medals2;
  const [cat, setCat] = useState<number | null>(null);
  const [only, setOnly] = useState("all");
  const [sort, setSort] = useState("rarity");

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
      copy.sort(
        (a, b) => b.bonus_addition_factor - a.bonus_addition_factor,
      );
    }
    return copy;
  }, [medals, cat, only, sort, rarities]);

  const nBuyable = medals.filter(
    (m) => m.get_type === 1 && m.price !== null,
  ).length;
  const nOwned = medals.filter((m) => m.owned).length;

  const getTypeText = (m: Medal) =>
    m.get_type === 1
      ? g.gtExchange.replace("{magic}", currency)
      : m.get_type === 2
        ? g.gtGrant
        : g.gtSynthesize;

  return (
    <div className="flex flex-col gap-3">
      <div className="pgtools">
        <div className="tide-tabs" role="tablist">
          <button
            type="button"
            role="tab" className="tide-tab"
            aria-selected={cat === null}
            onClick={() => setCat(null)}
          >
            {t.groupAll} {medals.length}
          </button>
          {cats.map(([id, name]) => (
            <button
              key={id}
              type="button"
              role="tab" className="tide-tab"
              aria-selected={cat === id}
              onClick={() => setCat(id)}
            >
              {name}{" "}
              {medals.filter((m) => m.category_id === id).length}
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
      </div>

      {list.length === 0 && (
        <p className="baozi-panel p-4 text-sm text-sub">{t.emptyFilter}</p>
      )}

      <div className="hall-grid">
        {list.map((m) => (
          <article
            key={m.id}
            className={`mcard${m.owned ? " is-owned" : ""}`}
          >
            <div
              className="mcard-tier"
              data-tone={medalRarityTone(rarities, m.rarity)}
            />
            <div className="mcard-bd">
              <div className="mcard-top">
                <MedalBadge
                  name={m.name}
                  assetRef={m.asset_ref}
                  rarity={m.rarity}
                  list={rarities}
                  size="lg"
                  cap={
                    m.inventory != null
                      ? fmt(t.capLabel, { n: m.inventory })
                      : null
                  }
                />
                <div className="t">
                  <h3>{m.name}</h3>
                  <div className="mcard-flags">
                    <MedalRarityChip list={rarities} value={m.rarity} />
                    {m.owned && (
                      <span className="mtag is-owned">
                        {m.wearing ? t.tagWorn : t.tagOwned}
                      </span>
                    )}
                    {m.limited && (
                      <span className="mtag is-limited">{t.limited}</span>
                    )}
                  </div>
                </div>
              </div>
              {m.description && (
                <p className="mcard-desc">{m.description}</p>
              )}
              <ul className="spec">
                <li>
                  <span>{g.getType}</span>
                  <b>{getTypeText(m)}</b>
                </li>
                <li>
                  <span>{g.duration}</span>
                  <b>
                    {m.duration_days
                      ? fmt(g.days, { n: m.duration_days })
                      : g.permanent}
                  </b>
                </li>
                <li>
                  <span>
                    {m.inventory != null ? t.specStockLeft : t.specStock}
                  </span>
                  <b>{m.inventory ?? t.stockUnlimited}</b>
                </li>
                <li>
                  <span>{t.specBonus}</span>
                  <b
                    className={
                      m.bonus_addition_factor > 0 ? "is-bonus" : ""
                    }
                  >
                    {m.bonus_addition_factor > 0
                      ? `+${m.bonus_addition_factor.toFixed(2)}%`
                      : t.noBonus}
                  </b>
                </li>
                {m.sale_end_at && (
                  <li>
                    <span>{g.saleUntil}</span>
                    <b suppressHydrationWarning>
                      {isoDay(m.sale_end_at, dateLocale(locale))}
                    </b>
                  </li>
                )}
              </ul>
            </div>
            <div className="mcard-ft">
              <div className="pricetag">
                {m.price ? (
                  <>
                    <b>{m.price.toLocaleString(dateLocale(locale))}</b>
                    <span>{currency}</span>
                  </>
                ) : (
                  <>
                    <b>—</b>
                    <span>{t.notForSale}</span>
                  </>
                )}
              </div>
              <MedalActions
                medalId={m.id}
                owned={m.owned}
                wearing={m.wearing}
                price={m.price}
                getType={m.get_type}
              />
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}
