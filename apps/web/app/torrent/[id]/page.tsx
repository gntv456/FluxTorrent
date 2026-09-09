import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { categoryColor, formatBytes, promotionBadge } from "@/lib/format";
import { DownloadButton } from "@/components/download-button";
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
                <span className={`sticker ${promo.className}`}>{promo.label}</span>
              )}
              {t.official && (
                <span className="sticker bg-indigo text-white">官种</span>
              )}
            </div>
            {t.small_descr && <p className="mt-1 text-sub">{t.small_descr}</p>}
            <dl className="num mt-3 grid grid-cols-2 gap-x-8 gap-y-1 text-sm md:grid-cols-4">
              <div className="flex justify-between gap-2">
                <dt className="text-sub">大小</dt>
                <dd>{formatBytes(t.size)}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">做种</dt>
                <dd className="text-mint">{t.seeders}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">下载</dt>
                <dd className="text-coral">{t.leechers}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">完成</dt>
                <dd>{t.times_completed}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">发布人</dt>
                <dd>{t.anonymous ? "匿名" : (t.owner_name ?? "—")}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">发布时间</dt>
                <dd>{new Date(t.created_at).toLocaleString("zh-CN")}</dd>
              </div>
            </dl>
          </div>
        </div>
        <DownloadButton torrentId={t.id} name={t.name} />
      </header>
    </article>
  );
}
