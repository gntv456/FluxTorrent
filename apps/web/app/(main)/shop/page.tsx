import { getShopItems, getMedals } from "@/lib/data";
import { BuyButton } from "@/components/buy-button";
import { MedalActions } from "@/components/medal-actions";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";

export const dynamic = "force-dynamic";

const RARITY_LABEL: Record<string, string> = {
  legendary: "传说",
  epic: "史诗",
  rare: "稀有",
  common: "普通",
};

/** 魔力商店（mybonus.php 口径）：勋章兑换区 + 道具区 */
export default async function ShopPage() {
  const { dict, locale } = await getDict();
  const items = await getShopItems();
  const medals = (await getMedals()).filter((m) => m.price !== null);
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.shop.title}</h1>
        <span className="text-sm text-sub">{dict.shop.subtitle}</span>
      </div>

      {/* 勋章兑换区（好学站：魔力可换勋章） */}
      {medals.length > 0 && (
        <section className="flex flex-col gap-3">
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">
                  <h2 className="font-display">{dict.shop.medalZone}</h2>
                </td>
              </tr>
            </tbody>
          </table>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {medals.map((m) => (
              <div
                key={m.id}
                className="flex flex-col gap-2 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
              >
                <div className="flex items-center justify-between">
                  <span aria-hidden className="text-3xl">🏅</span>
                  {m.rarity && (
                    <span className="sticker bg-sky text-white">{RARITY_LABEL[m.rarity] ?? m.rarity}</span>
                  )}
                </div>
                <h3 className="font-display text-base">{m.name}</h3>
                {m.description && <p className="text-xs text-sub">{m.description}</p>}
                <p className="num text-sm font-bold text-[var(--baozi-orange-dark)]">
                  ✨ {m.price?.toLocaleString(dateLocale(locale))}
                  {m.bonus_addition_factor > 0 && (
                    <span className="ml-1 text-xs font-normal text-sub">
                      +{m.bonus_addition_factor}%{dict.shop.bonusSuffix}
                    </span>
                  )}
                </p>
                <MedalActions medalId={m.id} owned={m.owned} wearing={m.wearing} price={m.price} />
              </div>
            ))}
          </div>
        </section>
      )}

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
      </section>

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        {dict.shop.ledgerNote}
      </p>
    </div>
  );
}
