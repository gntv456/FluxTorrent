import { getShopItems, getMySpark } from "@/lib/data";
import { ShopCatalog } from "@/components/shop-catalog";
import { VoucherPanel } from "@/components/voucher-panel";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 魔力商店（mybonus.php 口径）：道具区。改版要点（2026-09-23）：
 *  余额条（读 /me 的 spark_balance，零 API 改动）+ kind 归 5 组 + 单位价格 + 四态购买。
 *  勋章兑换仍在 /medals，不放商店。 */
export default async function ShopPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("shop");
  if (gate) return gate;

  const { dict, currency, locale } = await getDict();
  const [items, spark] = await Promise.all([getShopItems(), getMySpark()]);
  const magic = (s: string) => s.replaceAll("{magic}", currency);
  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Magic Shop</div>
          <h1 className="font-display text-2xl">
            {magic(dict.shop.title)}
          </h1>
        </div>
        <span className="sub">{magic(dict.shop.subtitle)}</span>
      </div>

      <div className="shop-bal">
        <div className="flex flex-wrap items-baseline gap-2">
          <span className="k">{magic(dict.shop.myBalance)}</span>
          <span className="v">
            {spark === null ? "—" : spark.toLocaleString(dateLocale(locale))}
          </span>
          <span className="u">{dict.shop.balanceNote}</span>
        </div>
        <div className="tail">
          <a className="btn btn-sm btn-ghost" href="/my-spark">
            {magic(dict.shop.earnMore)}
          </a>
        </div>
      </div>

      <ShopCatalog items={items} balance={spark} />

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        {magic(dict.shop.ledgerNote)}
      </p>

      {/* 我的券（0073：免费券/中性券库存与使用） */}
      <VoucherPanel />
    </div>
  );
}
