import Link from "next/link";
import type { Dict } from "@/i18n/zh-CN";
import { PANEL_MD } from "@/lib/ui-classes";
import { UnsubscribeButton } from "@/components/unsubscribe-button";

/** 追更中心「出品方/字幕组」档（P-588）。
 *  两类共用 network_subscriptions，行按 kind 链到各自详情页。 */
export interface NetSubRow {
  network_id: number;
  name: string;
  kind: string;
  torrents: number;
}

const ROW_CLS =
  "flex flex-wrap items-baseline justify-between gap-2 " +
  "rounded-lg border border-line bg-card px-4 py-3";

export function NetworkSubs({
  rows,
  dict,
}: {
  rows: NetSubRow[];
  dict: Dict;
}) {
  const t = dict.subscriptions;
  return (
    <section className={PANEL_MD} data-network-subs>
      <h2 className="font-bold">{t.tabNetworks}</h2>
      {rows.length === 0 ? (
        <p className="py-6 text-center text-sub">{t.networksEmpty}</p>
      ) : (
        <ul className="mt-2 flex list-none flex-col gap-2 p-0">
          {rows.map((r) => (
            <li key={r.network_id} data-network-id={r.network_id}>
              <div className={ROW_CLS}>
                <Link
                  href={
                    r.kind === "subtitle_group"
                      ? `/subtitle-groups/${r.network_id}`
                      : `/networks/${r.network_id}`
                  }
                  className="min-w-0 flex-1 truncate font-bold"
                >
                  {r.name}
                </Link>
                <span className="text-xs text-sub">
                  {r.kind === "subtitle_group"
                    ? dict.subtitleGroups.title
                    : dict.networks.title}
                  {" · "}
                  {t.releases} {r.torrents}
                </span>
                <UnsubscribeButton networkId={r.network_id} />
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
