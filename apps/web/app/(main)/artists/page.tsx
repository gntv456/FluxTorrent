import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";

export const dynamic = "force-dynamic";

const CARD_CLS =
  "block rounded-xl border border-line bg-card p-4 " +
  "transition-transform hover:-translate-y-0.5";

interface ArtistItem {
  id: number;
  name: string;
  torrents: number;
}

/** 自由值实体总览页（0327 music 批；0338 通用化）。
 *  后端 /artists 系列已交付（列表/实体页/榜单），本页补上入口——
 *  此前只有 API，用户点不到。形态与 /networks 一致（卡片墙 + 收录数）。
 *  0338 起 `?kind=` 参数化：artist（音乐艺人）/ author（电子书作者）/
 *  studio（动漫制作公司）复用同一页面。 */
export default async function ArtistsPage({
  searchParams,
}: {
  searchParams: Promise<{ kind?: string }>;
}) {
  const { dict } = await getDict();
  const sp = await searchParams;
  const kind = (sp?.kind ?? "artist").trim().toLowerCase() || "artist";
  let items: ArtistItem[] = [];
  let failed = false;
  try {
    const r = await api.get<{ items: ArtistItem[] }>(
      `/api/v1/artists?limit=100&kind=${encodeURIComponent(kind)}`,
    );
    items = r.items ?? [];
  } catch {
    failed = true;
  }
  const t = dict.artists;

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
        {items.map((a) => (
          <li key={a.id} data-artist-id={a.id}>
            <Link href={`/artists/${a.id}`} className={CARD_CLS}>
              <div className="flex items-center justify-between gap-2">
                <b className="clamp2 min-w-0 flex-1">{a.name}</b>
                <span className="shrink-0 text-xs text-sub">
                  {t.countPrefix}
                  {a.torrents}
                </span>
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
