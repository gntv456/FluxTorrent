import Link from "next/link";
import { getRequests } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";

export const dynamic = "force-dynamic";

/** 求种区（NexusPHP 经典表格：colhead 表头 + rowfollow 行） */
export default async function RequestsPage() {
  const { dict, locale } = await getDict();
  const requests = await getRequests();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.requests.title}</h1>
      {requests.length === 0 ? (
        <p className="py-10 text-center text-sub">{dict.requests.empty}</p>
      ) : (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.requests.colName}</td>
              <td className="colhead w-24 text-right">{dict.requests.colBounty}</td>
              <td className="colhead hidden w-32 sm:table-cell">{dict.requests.colRequester}</td>
              <td className="colhead w-24 text-right">{dict.requests.colStatus}</td>
            </tr>
          </thead>
          <tbody>
            {requests.map((r) => (
              <tr key={r.id}>
                <td>
                  <span className="font-bold text-ink">{r.title}</span>
                  {r.descr && <p className="mt-1 text-xs text-sub">{r.descr}</p>}
                </td>
                <td className="num text-right font-bold text-[var(--baozi-orange-dark)]">
                  {r.bounty > 0
                    ? r.bounty.toLocaleString(dateLocale(locale))
                    : "—"}
                </td>
                <td className="hidden text-sub sm:table-cell">
                  {r.username ?? "—"}
                </td>
                <td className="text-right">
                  {r.fulfilled_torrent_id ? (
                    <Link
                      href={`/torrent/${r.fulfilled_torrent_id}`}
                      className="font-bold text-sky hover:text-[var(--baozi-orange)]"
                    >
                      {dict.requests.fulfilled}
                    </Link>
                  ) : (
                    <span className="text-sub">{dict.requests.pending}</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
