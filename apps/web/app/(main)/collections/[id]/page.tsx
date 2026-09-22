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

interface CollectionDetail {
  id: number;
  kind: string;
  name: string;
  descr: string | null;
  cover: string | null;
  items: {
    id: number;
    name: string;
    size: number;
    seeders: number;
    leechers: number;
    times_completed: number;
  }[];
}

/** 合集详情页（0157）：元数据 + 收录种列表（标题行点击进详情） */
export default async function CollectionDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const cid = Number(id);
  if (!Number.isFinite(cid)) notFound();
  let c: CollectionDetail;
  try {
    c = await api.get<CollectionDetail>(
      `/api/v1/collections/${encodeURIComponent(cid)}`,
    );
  } catch {
    notFound();
  }
  const { dict } = await getDict();
  const t = dict.collections;

  return (
    <div className="flex flex-col gap-4">
      <nav className="text-sm text-sub" aria-label="breadcrumb">
        <Link href="/collections">{t.title}</Link>
        <span aria-hidden> › </span>
        <span className="text-body">{c.name}</span>
      </nav>
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">{c.name}</h1>
        <span
          className={`sticker ${
            c.kind === "series" ? "bg-indigo text-white" : "bg-sun text-ink"
          }`}
        >
          {c.kind === "series" ? t.seriesTag : t.collectionTag}
        </span>
      </div>
      {c.descr && <p className="text-sm text-sub">{c.descr}</p>}
      <ul className="flex list-none flex-col gap-2 p-0">
        {c.items.map((it) => (
          <li key={it.id} data-torrent-id={it.id}>
            <Link
              href={`/torrent/${it.id}`}
              className={ITEM_CLS}
            >
              <span className={TITLE_CLS}>{it.name}</span>
              <span className="text-xs text-sub">
                {" "}
                🌱 {it.seeders} · ⬇️ {it.leechers} · ✅ {it.times_completed}
                {" · "}
                {formatBytes(it.size)}
              </span>
            </Link>
          </li>
        ))}
      </ul>
      {c.items.length === 0 && (
        <p className="py-8 text-center text-sub">{t.emptyItems}</p>
      )}
    </div>
  );
}
