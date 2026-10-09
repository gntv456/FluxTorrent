import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const CARD_CLS =
  "block rounded-xl border border-line bg-card p-4 " +
  "transition-transform hover:-translate-y-0.5";

interface NetworkItem {
  id: number;
  name: string;
  torrents: number;
}

/** 出品方总览页（0330 documentary 专项 P1）。
 *  network 维度此前只能筛不能聚合——点不出「BBC 的全部纪录片」。
 *  本页是聚合入口：卡片墙 + 名下收录数，点进 /networks/{id} 看全部种。
 *  通用层：kind=network 为纪录片出品方，将来 movie/anime 的
 *  studio（制片厂/动画制作）复用同一页与同一表。 */
export default async function NetworksPage() {
  const { dict } = await getDict();
  let items: NetworkItem[] = [];
  let failed = false;
  try {
    const r = await api.get<{ items: NetworkItem[] }>(
      "/api/v1/networks?limit=100",
    );
    items = r.items ?? [];
  } catch {
    failed = true;
  }
  const t = dict.networks;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>
      {failed && <p className="text-sm text-sub">{t.loadFailed}</p>}
      {!failed && items.length === 0 && (
        <p className="py-8 text-center text-sub">{t.empty}</p>
      )}
      <ul className="grid list-none gap-3 p-0 sm:grid-cols-2 xl:grid-cols-3">
        {items.map((n) => (
          <li key={n.id} data-network-id={n.id}>
            <Link href={`/networks/${n.id}`} className={CARD_CLS}>
              <div className="flex items-center justify-between gap-2">
                <b className="clamp2 min-w-0 flex-1">{n.name}</b>
                <span className="shrink-0 text-xs text-sub">
                  {t.countPrefix}
                  {n.torrents}
                </span>
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
