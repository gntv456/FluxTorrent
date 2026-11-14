"use client";

import { useState } from "react";
import type { Medal } from "@fluxtorrent/domain-types";
import { dateLocale, fmt } from "@/i18n/config";
import { medalRarityTone, type MedalRarity } from "@/lib/medal-rarity";
import { MedalBadge } from "@/components/medal-badge";
import { MedalRarityChip } from "@/components/medal-rarity-chip";
import { MedalActions } from "@/components/medal-actions";
import type { Dict } from "@/i18n/zh-CN";

/** 勋章卡（从 medal-hall.tsx 按域拆出，300 行门禁）。
 *
 *  2026-11-02 手机端改造：规格表（获取方式/价格/有效期/库存/加成/截止 共六行）
 *  在窄屏上摊开会把单卡拉到约 440px，二十枚就是十屏。窄屏改成**一行摘要**
 *  （获取方式 · 价格 · 加成），点「详情」才展开完整规格；桌面端仍是原表。
 *  规格表只有一份 DOM，靠外层包裹切换显示，不复制。 */

/** 固定 UTC 格式化：两端一致，避免 SSR/CSR 跨时区渲染出不同日期 */
export function isoDay(iso: string, locale: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString(locale, { timeZone: "UTC" });
}

/** 价格行文案（本地化数字 + 站点货币名） */
function priceText(m: Medal, locale: string, currency: string): string {
  return `${m.price?.toLocaleString(locale)} ${currency}`;
}

export function MedalCard({
  m,
  rarities,
  t,
  g,
  locale,
  currency,
  balance,
  globalGiftTaxBp,
  onWearChange,
}: {
  m: Medal;
  rarities: MedalRarity[];
  t: Dict["medals"];
  g: Dict["medals2"];
  /** 站点语言（dateLocale 认的那几个） */
  locale: Parameters<typeof dateLocale>[0];
  currency: string;
  balance?: number | null;
  globalGiftTaxBp?: number;
  onWearChange: () => void;
}) {
  const [open, setOpen] = useState(false);
  const getTypeText =
    m.get_type === 1
      ? g.gtExchange.replace("{magic}", currency)
      : m.get_type === 2
        ? g.gtGrant
        : g.gtSynthesize;
  const price = m.price
    ? priceText(m, dateLocale(locale), currency)
    : t.notForSale;
  const bonus =
    m.bonus_addition_factor > 0
      ? `+${m.bonus_addition_factor.toFixed(2)}%`
      : t.noBonus;
  return (
    <article className={`mcard${m.owned ? " is-owned" : ""}`}>
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
        {m.description && <p className="mcard-desc">{m.description}</p>}
        {/* 窄屏：一行摘要 + 详情开关 */}
        <div className="mcard-sum flex sm:hidden">
          <span>{getTypeText}</span>
          <b className={m.price ? "is-price" : ""}>{price}</b>
          <b className={m.bonus_addition_factor > 0 ? "is-bonus" : ""}>
            {bonus}
          </b>
          <button
            type="button"
            className="mcard-sum__more"
            onClick={() => setOpen((v) => !v)}
            aria-expanded={open}
          >
            {open ? t.hideSpec : t.showSpec} ›
          </button>
        </div>
        {/* 规格表：桌面常驻；窄屏点「详情」展开（只有这一份 DOM） */}
        <div className={open ? "block sm:contents" : "hidden sm:contents"}>
          <ul className="spec">
            <li>
              <span>{g.getType}</span>
              <b>{getTypeText}</b>
            </li>
            <li>
              <span>{g.priceLabel}</span>
              <b className={m.price ? "is-price" : ""}>{price}</b>
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
              <b className={m.bonus_addition_factor > 0 ? "is-bonus" : ""}>
                {bonus}
              </b>
            </li>
            {m.sale_begin_at && (
              <li>
                <span>{g.saleFrom}</span>
                <b suppressHydrationWarning>
                  {isoDay(m.sale_begin_at, locale)}
                </b>
              </li>
            )}
            {m.sale_end_at && (
              <li>
                <span>{g.saleUntil}</span>
                <b suppressHydrationWarning>
                  {isoDay(m.sale_end_at, locale)}
                </b>
              </li>
            )}
          </ul>
        </div>
      </div>
      <div className="mcard-ft">
        <MedalActions
          medalId={m.id}
          owned={m.owned}
          wearing={m.wearing}
          price={m.price}
          getType={m.get_type}
          saleStatus={m.sale_status}
          balance={balance}
          giftFeeBp={m.gift_fee_bp ?? null}
          globalGiftTaxBp={globalGiftTaxBp}
          onWearChange={onWearChange}
        />
      </div>
    </article>
  );
}
