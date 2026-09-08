import Link from "next/link";
import { getRequests } from "@/lib/data";

export const dynamic = "force-dynamic";

export default async function RequestsPage() {
  const requests = await getRequests();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">求种区</h1>
        <span className="text-sm text-sub">说出你想要的资源，悬赏求种</span>
      </div>
      {requests.length === 0 ? (
        <p className="py-10 text-center text-sub">暂无求种</p>
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
                    悬赏 {r.bounty.toLocaleString("zh-CN")}
                  </span>
                )}
              </div>
              {r.descr && <p className="mt-1 text-sm text-sub">{r.descr}</p>}
              <p className="mt-1 text-xs text-sub">
                求种人 {r.username ?? "—"}
                {r.fulfilled_torrent_id ? (
                  <>
                    {" · "}
                    <Link
                      href={`/torrent/${r.fulfilled_torrent_id}`}
                      className="text-sky"
                    >
                      已应种 →
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
