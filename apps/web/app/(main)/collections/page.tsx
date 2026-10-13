import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { ErrorPanel, normalizeError } from "@/components/ui/error-panel";
import { EmptyState } from "@/components/ui/empty-state";
import { SkeletonRows } from "@/components/ui/skeleton";

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
  let items: CollectionItem[] | null = null;
  let loadErr: unknown = null;
  try {
    items = await api.get<CollectionItem[]>("/api/v1/collections");
  } catch (e) {
    loadErr = e;
  }
  const t = dict.collections;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>
      {/* A3 四态：RSC 侧手写三态段落 → ErrorPanel / EmptyState / Skeleton */}
      {loadErr !== null && <ErrorPanel error={normalizeError(loadErr)} />}
      {loadErr === null && items === null && (
        <SkeletonRows rows={4} height={96} />
      )}
      {loadErr === null && items !== null && items.length === 0 && (
        <EmptyState icon="info" title={t.empty} />
      )}
      <ul className="grid list-none gap-3 p-0 sm:grid-cols-2 xl:grid-cols-3">
        {(items ?? []).map((c) => (
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
