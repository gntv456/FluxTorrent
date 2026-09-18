import { getShopItems } from "@/lib/data";
import { BuyButton } from "@/components/buy-button";
import { VoucherPanel } from "@/components/voucher-panel";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 魔力商店（mybonus.php 口径）：仅道具区（勋章兑换在 /medals 勋章页，不放商店） */
export default async function ShopPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("shop");
  if (gate) return gate;

  const { dict, currency, locale } = await getDict();
  const items = await getShopItems();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.shop.title.replace("{magic}", currency)}</h1>
        <span className="text-sm text-sub">{dict.shop.subtitle.replace("{magic}", currency)}</span>
      </div>

      {/* 道具区 */}
      <section className="flex flex-col gap-3">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">
                <h2 className="font-display">{dict.shop.itemZone}</h2>
              </td>
            </tr>
          </tbody>
        </table>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {items.map((item) => (
            <div
              key={item.id}
              className="flex flex-col gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
            >
              <div className="flex items-center justify-between">
                <h2 className="font-bold">{item.name}</h2>
                <span className="sticker bg-sun text-ink num">
                  {item.price.toLocaleString(dateLocale(locale))}
                </span>
              </div>
              <p className="text-xs text-sub">
                {item.kind === "upload_credit"
                  ? dict.shop.uploadCredit
                  : item.kind === "voucher_free"
                    ? dict.shop.voucherFree
                    : item.kind === "voucher_neutral"
                      ? dict.shop.voucherNeutral
                      : dict.shop.sitePerk}
              </p>
              <BuyButton itemId={item.id} name={item.name} price={item.price} />
            </div>
          ))}
        </div>
      </section>

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        {dict.shop.ledgerNote}
      </p>

      {/* 我的券（0073：免费券/中性券库存与使用） */}
      <VoucherPanel />
    </div>
  );
}
