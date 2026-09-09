import { getMedals } from "@/lib/data";
import { MedalActions } from "@/components/medal-actions";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

const RARITY_STYLE: Record<string, string> = {
  legendary: "bg-sun text-ink",
  epic: "bg-indigo text-white",
  rare: "bg-sky text-white",
  common: "bg-mint text-white",
};

export default async function MedalsPage() {
  const { dict, locale } = await getDict();
  const medals = await getMedals();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.medals.title}</h1>
        <span className="text-sm text-sub">{dict.medals.subtitle}</span>
      </div>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        {medals.map((m) => (
          <div
            key={m.id}
            className="flex flex-col gap-2 rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
          >
            <div className="flex items-center justify-between">
              <span aria-hidden className="text-3xl">
                🏅
              </span>
              {m.rarity && (
                <span className={`sticker ${RARITY_STYLE[m.rarity] ?? "bg-sky text-white"}`}>
                  {m.rarity}
                </span>
              )}
            </div>
            <h2 className="font-display text-lg">{m.name}</h2>
            <p className="num text-xs text-sub">
              {m.price
                ? fmt(dict.medals.sparkPrice, { n: m.price.toLocaleString(dateLocale(locale)) })
                : dict.medals.notForSale}
              {m.limited ? ` · ${dict.medals.limited}` : ""}
            </p>
            <MedalActions medalId={m.id} owned={m.owned} wearing={m.wearing} price={m.price} />
          </div>
        ))}
      </div>
    </div>
  );
}
