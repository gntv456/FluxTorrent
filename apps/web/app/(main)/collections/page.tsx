import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const CARD_CLS =
  "block rounded-xl border border-line bg-card p-4 " +
  "transition-transform hover:-translate-y-0.5";

interface CollectionItem {
  id: number;
  kind: string;
  name: string;
  descr: string | null;
  cover: string | null;
  count: number;
}

/** 合集/系列总览页（0157 阶段三聚合层）：
 *  kind=collection 策展合集 / kind=series 作品系列，卡片墙展示 */
export default async function CollectionsPage() {
  const { dict } = await getDict();
  let items: CollectionItem[] = [];
  let failed = false;
  try {
    items = await api.get<CollectionItem[]>("/api/v1/collections");
  } catch {
    failed = true;
  }
  const t = dict.collections;

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
        {items.map((c) => (
          <li key={c.id} data-collection-id={c.id}>
            <Link
              href={`/collections/${c.id}`}
              className={CARD_CLS}
            >
              <div className="flex items-center justify-between">
                <b className="clamp2">{c.name}</b>
                <span
                  className={`sticker ${
                    c.kind === "series"
                      ? "bg-indigo text-white"
                      : "bg-sun text-ink"
                  }`}
                >
                  {c.kind === "series" ? t.seriesTag : t.collectionTag}
                </span>
              </div>
              {c.descr && (
                <p className="mt-1 line-clamp-2 text-xs text-sub">{c.descr}</p>
              )}
              <p className="mt-2 text-xs text-sub">
                {t.countPrefix}
                {c.count}
              </p>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
