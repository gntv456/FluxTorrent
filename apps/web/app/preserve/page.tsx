import { getPreserve } from "@/lib/data";
import { formatBytes } from "@/lib/format";
import Link from "next/link";

export const dynamic = "force-dynamic";

export default async function PreservePage() {
  const items = await getPreserve();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">保种区</h1>
        <span className="text-sm text-sub">免费下载 · 做种 &gt; 7 自动移出</span>
      </div>
      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
        保种区种子享受免费下载；做种人数超过 7 后自动移出，免费状态延续 3 天。
      </p>
      {items.length === 0 ? (
        <p className="py-10 text-center text-sub">暂无待保种资源</p>
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
                    {p.claimed_by ? `认领人 ${p.claimed_by}` : "待认领"}
                  </p>
                </div>
                <div className="num flex shrink-0 flex-col items-end text-xs">
                  <span className="text-sub">{formatBytes(p.size)}</span>
                  <span className="text-mint">做种 {p.seeders}</span>
                </div>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
