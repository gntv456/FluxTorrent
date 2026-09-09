import { getShopItems } from "@/lib/data";
import { BuyButton } from "@/components/buy-button";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function ShopPage() {
  const { dict, locale } = await getDict();
  const items = await getShopItems();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.shop.title}</h1>
        <span className="text-sm text-sub">{dict.shop.subtitle}</span>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {items.map((item) => (
          <div
            key={item.id}
            className="flex flex-col gap-2 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
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
                : dict.shop.sitePerk}
            </p>
            <BuyButton itemId={item.id} name={item.name} price={item.price} />
          </div>
        ))}
      </div>
    </div>
  );
}
