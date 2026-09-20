import { getMedals, getMedalRarities } from "@/lib/data";
import { MedalActions } from "@/components/medal-actions";
import { MedalIcon } from "@/components/medal-icon";
import { medalRarityLabel, medalRarityStyle } from "@/lib/medal-rarity";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt, fmtCur } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 勋章殿堂（好学站 medal.php 口径）：按分类分组，卡片含描述/获取方式/有效期/库存/销售期/加成 */
export default async function MedalsPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("medals");
  if (gate) return gate;

  const { dict, locale, currency } = await getDict();
  const medals = await getMedals();
  // 稀有度词表（0143，站长可在后台维护）；角标标签与配色都按它渲染
  const rarities = await getMedalRarities();
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
                  <MedalIcon src={m.asset_ref} size={40} title={m.name} />
                  {m.rarity && (
                    <span className={`sticker ${medalRarityStyle(rarities, m.rarity)}`}>
                      {medalRarityLabel(rarities, m.rarity)}
                    </span>
                  )}
                </div>
                <h3 className="font-display text-lg">{m.name}</h3>
                {m.description && <p className="text-xs leading-relaxed text-sub">{m.description}</p>}
                <p className="num text-xs text-sub">
                  {m.price
                    ? fmtCur(dict.medals.sparkPrice, { n: m.price.toLocaleString(dateLocale(locale)) }, currency)
                    : dict.medals.notForSale}
                  {m.limited ? ` · ${dict.medals.limited}` : ""}
                </p>
                <ul className="medal-meta">
                  <li>
                    {t.getType}: {m.get_type === 1 ? t.gtExchange.replace("{magic}", currency) : m.get_type === 2 ? t.gtGrant : t.gtSynthesize}
                  </li>
                  <li>
                    {t.duration}:{" "}
                    {m.duration_days === null || m.duration_days === 0 ? t.permanent : fmt(t.days, { n: m.duration_days })}
                  </li>
                  {m.inventory !== null && <li>{fmt(t.inventory, { n: m.inventory })}</li>}
                  {m.bonus_addition_factor > 0 && (
                    <li className="text-[var(--baozi-orange-dark)]">
                      {fmtCur(t.bonusAddition, { n: m.bonus_addition_factor }, currency)}
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
