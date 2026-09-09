import Link from "next/link";
import { getRequests } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function RequestsPage() {
  const { dict, locale } = await getDict();
  const requests = await getRequests();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.requests.title}</h1>
        <span className="text-sm text-sub">{dict.requests.subtitle}</span>
      </div>
      {requests.length === 0 ? (
        <p className="py-10 text-center text-sub">{dict.requests.empty}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {requests.map((r) => (
            <li
              key={r.id}
              className="rounded-[var(--r-md)] border border-line bg-white p-4 shadow-[var(--shadow-card)]"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h2 className="font-bold">{r.title}</h2>
                {r.bounty > 0 && (
                  <span className="sticker bg-sun text-ink num">
                    {fmt(dict.requests.bounty, {
                      n: r.bounty.toLocaleString(dateLocale(locale)),
                    })}
                  </span>
                )}
              </div>
              {r.descr && <p className="mt-1 text-sm text-sub">{r.descr}</p>}
              <p className="mt-1 text-xs text-sub">
                {fmt(dict.requests.requester, { name: r.username ?? "—" })}
                {r.fulfilled_torrent_id ? (
                  <>
                    {" · "}
                    <Link
                      href={`/torrent/${r.fulfilled_torrent_id}`}
                      className="text-sky"
                    >
                      {dict.requests.fulfilled}
                    </Link>
                  </>
                ) : null}
              </p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
