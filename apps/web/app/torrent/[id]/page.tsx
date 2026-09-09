import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { categoryColor, formatBytes, promotionBadge } from "@/lib/format";
import { DownloadButton } from "@/components/download-button";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

export default async function TorrentDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const tid = Number(id);
  if (!Number.isFinite(tid)) notFound();
  let t: TorrentListItem;
  try {
    t = await api.get<TorrentListItem>(`/api/v1/torrents/${encodeURIComponent(tid)}`);
  } catch {
    notFound();
  }

  const { dict, locale } = await getDict();
  const promo = promotionBadge(t.promotion);

  return (
    <article className="flex flex-col gap-4">
      <header className="rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]">
        <div className="flex items-start gap-4">
          <span
            aria-hidden
            className="h-[64px] w-[64px] shrink-0 rounded-[var(--r-md)]"
            style={{ background: categoryColor(t.category_id) }}
          />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="font-display text-2xl break-all">{t.name}</h1>
              {promo && (
                <span className={`sticker ${promo.className}`}>
                  {dict.promotion[promo.key]}
                </span>
              )}
              {t.official && (
                <span className="sticker bg-indigo text-white">
                  {dict.torrent.official}
                </span>
              )}
            </div>
            {t.small_descr && <p className="mt-1 text-sub">{t.small_descr}</p>}
            <dl className="num mt-3 grid grid-cols-2 gap-x-8 gap-y-1 text-sm md:grid-cols-4">
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.size}</dt>
                <dd>{formatBytes(t.size)}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.seeding}</dt>
                <dd className="text-mint">{t.seeders}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.leeching}</dt>
                <dd className="text-coral">{t.leechers}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.completed}</dt>
                <dd>{t.times_completed}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.uploader}</dt>
                <dd>{t.anonymous ? dict.torrent.anonymous : (t.owner_name ?? "—")}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.uploadedAt}</dt>
                <dd>
                  {new Date(t.created_at).toLocaleString(dateLocale(locale))}
                </dd>
              </div>
            </dl>
          </div>
        </div>
        <DownloadButton torrentId={t.id} name={t.name} />
      </header>
    </article>
  );
}
