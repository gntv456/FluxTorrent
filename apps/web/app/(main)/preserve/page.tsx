import { getPreserve } from "@/lib/data";
import { formatBytes } from "@/lib/format";
import Link from "next/link";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function PreservePage() {
  const { dict } = await getDict();
  const items = await getPreserve();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.preserve.title}</h1>
        <span className="text-sm text-sub">{dict.preserve.subtitle}</span>
      </div>
      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
        {dict.preserve.rule}
      </p>
      {items.length === 0 ? (
        <p className="py-10 text-center text-sub">{dict.preserve.empty}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {items.map((p) => (
            <li key={p.torrent_id}>
              <Link
                href={`/torrent/${p.torrent_id}`}
                className="flex items-center justify-between rounded-[var(--r-md)] border border-line bg-white p-3 shadow-[var(--shadow-card)] hover:-translate-y-0.5"
              >
                <div className="min-w-0">
                  <p className="truncate font-bold">{p.name}</p>
                  <p className="text-xs text-sub">
                    {p.claimed_by
                      ? fmt(dict.preserve.claimedBy, { name: p.claimed_by })
                      : dict.preserve.unclaimed}
                  </p>
                </div>
                <div className="num flex shrink-0 flex-col items-end text-xs">
                  <span className="text-sub">{formatBytes(p.size)}</span>
                  <span className="text-mint">
                    {fmt(dict.preserve.seeding, { n: p.seeders })}
                  </span>
                </div>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
