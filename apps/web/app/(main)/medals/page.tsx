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

const RARITY_LABEL: Record<string, string> = {
  legendary: "传说",
  epic: "史诗",
  rare: "稀有",
  common: "普通",
};

/** 勋章殿堂（好学站 medal.php 口径）：按分类分组，卡片含描述/获取方式/有效期/库存/销售期/加成 */
export default async function MedalsPage() {
  const { dict, locale } = await getDict();
  const medals = await getMedals();
  const t = dict.medals2;
  // 分类分组（0 = 未分组）
  const groups = new Map<number, { name: string; items: typeof medals }>();
  for (const m of medals) {
    const key = m.category_id;
    const g = groups.get(key) ?? { name: m.category_name ?? t.uncategorized, items: [] };
    g.items.push(m);
    groups.set(key, g);
  }
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.medals.title}</h1>
        <span className="text-sm text-sub">{dict.medals.subtitle}</span>
      </div>
      {[...groups.entries()].map(([cid, g]) => (
        <section key={cid} className="flex flex-col gap-3">
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">
                  <h2 className="font-display">
                    {cid === 0 ? t.uncategorized : g.name}
                    <span className="ml-2 text-xs font-normal text-sub">
                      {fmt(t.groupCount, { n: g.items.length })}
                    </span>
                  </h2>
                </td>
              </tr>
            </tbody>
          </table>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {g.items.map((m) => (
              <div
                key={m.id}
                className="flex flex-col gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
              >
                <div className="flex items-center justify-between">
                  <span aria-hidden className="text-3xl">
                    🏅
                  </span>
                  {m.rarity && (
                    <span className={`sticker ${RARITY_STYLE[m.rarity] ?? "bg-sky text-white"}`}>
                      {RARITY_LABEL[m.rarity] ?? m.rarity}
                    </span>
                  )}
                </div>
                <h3 className="font-display text-lg">{m.name}</h3>
                {m.description && <p className="text-xs leading-relaxed text-sub">{m.description}</p>}
                <p className="num text-xs text-sub">
                  {m.price
                    ? fmt(dict.medals.sparkPrice, { n: m.price.toLocaleString(dateLocale(locale)) })
                    : dict.medals.notForSale}
                  {m.limited ? ` · ${dict.medals.limited}` : ""}
                </p>
                <ul className="medal-meta">
                  <li>
                    {t.getType}: {m.get_type === 1 ? t.gtExchange : m.get_type === 2 ? t.gtGrant : t.gtSynthesize}
                  </li>
                  <li>
                    {t.duration}:{" "}
                    {m.duration_days === null || m.duration_days === 0 ? t.permanent : fmt(t.days, { n: m.duration_days })}
                  </li>
                  {m.inventory !== null && <li>{fmt(t.inventory, { n: m.inventory })}</li>}
                  {m.bonus_addition_factor > 0 && (
                    <li className="text-[var(--baozi-orange-dark)]">
                      {fmt(t.bonusAddition, { n: m.bonus_addition_factor })}
                    </li>
                  )}
                  {m.sale_end_at && (
                    <li>
                      {t.saleUntil} {new Date(m.sale_end_at).toLocaleDateString(dateLocale(locale))}
                    </li>
                  )}
                </ul>
                <MedalActions medalId={m.id} owned={m.owned} wearing={m.wearing} price={m.price} />
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
