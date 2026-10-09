import Link from "next/link";
import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const ITEM_CLS =
  "flex flex-wrap items-baseline justify-between gap-2 " +
  "rounded-lg border border-line bg-card px-4 py-3 hover:border-sky";
const TITLE_CLS = "min-w-0 flex-1 truncate font-bold";

interface ArtistDetail {
  id: number;
  name: string;
  items: {
    id: number;
    name: string;
    size: number;
    seeders: number;
    times_completed: number;
  }[];
}

/** 艺人页（0327 补齐前端）：该艺人名下全部过审种。 */
export default async function ArtistDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const aid = Number(id);
  if (!Number.isFinite(aid)) notFound();
  let a: ArtistDetail;
  try {
    a = await api.get<ArtistDetail>(
      `/api/v1/artists/${encodeURIComponent(aid)}`,
    );
  } catch {
    notFound();
  }
  const { dict } = await getDict();
  const t = dict.artists;

  return (
    <div className="flex flex-col gap-4">
      <nav className="text-sm text-sub" aria-label="breadcrumb">
        <Link href="/artists">{t.backToList}</Link>
        <span aria-hidden> › </span>
        <span className="text-body">{a.name}</span>
      </nav>
      <h1 className="font-display text-2xl">{a.name}</h1>
      <ul className="flex list-none flex-col gap-2 p-0">
        {a.items.map((it) => (
          <li key={it.id} data-torrent-id={it.id}>
            <Link href={`/torrent/${it.id}`} className={ITEM_CLS}>
              <span className={TITLE_CLS}>{it.name}</span>
              <span className="text-xs text-sub">
                {" "}
                🌱 {it.seeders} · ✅ {it.times_completed}
                {" · "}
                {formatBytes(it.size)}
              </span>
            </Link>
          </li>
        ))}
      </ul>
      {a.items.length === 0 && (
        <p className="py-8 text-center text-sub">{t.emptyItems}</p>
      )}
    </div>
  );
}
