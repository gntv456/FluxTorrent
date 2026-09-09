import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { categoryColor, editionName, formatBytes, promotionBadge } from "@/lib/format";
import { DownloadButton } from "@/components/download-button";
import { TorrentSocial } from "@/components/torrent-social";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import type { TorrentComment, TorrentListItem } from "@fluxtorrent/domain-types";

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
  let comments: TorrentComment[] = [];
  try {
    t = await api.get<TorrentListItem>(`/api/v1/torrents/${encodeURIComponent(tid)}`);
  } catch {
    notFound();
  }
  // 评论加载失败不阻塞详情页
  try {
    comments = await api.get<TorrentComment[]>(
      `/api/v1/torrents/${encodeURIComponent(tid)}/comments`,
    );
  } catch {
    comments = [];
  }

  const { dict, locale } = await getDict();
  const promo = promotionBadge(t.promotion);
  const edition = editionName(t.edition_id);
  const grade =
    t.grade_id !== null ? dict.torrents.grades[t.grade_id + 1] : undefined;
  const medium = dict.torrents.media[t.medium_id] ?? String(t.medium_id);
  const category = dict.torrents.categories[t.category_id] ?? String(t.category_id);

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
              {t.sticky && (
                <span className="sticker bg-sun text-ink">
                  {dict.torrent.sticky}
                </span>
              )}
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
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.medium}</dt>
                <dd>{medium}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.grade}</dt>
                <dd>{grade ?? "—"}</dd>
              </div>
              {edition && (
                <div className="flex justify-between gap-2">
                  <dt className="text-sub">{dict.torrent.edition}</dt>
                  <dd>{edition}</dd>
                </div>
              )}
              <div className="flex justify-between gap-2">
                <dt className="text-sub">{dict.torrent.category}</dt>
                <dd>{category}</dd>
              </div>
            </dl>
            <p className="mt-2 break-all text-[11px] text-sub">
              {dict.torrent.infoHash}: {t.info_hash.trim()}
            </p>
          </div>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-3">
          <DownloadButton torrentId={t.id} name={t.name} />
          <TorrentSocial torrentId={t.id} />
        </div>
      </header>

      {/* 评论区（M07）：发表 + 列表 */}
      <section className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]">
        <h2 className="font-display text-lg">
          {dict.torrent.commentsTitle.replace("{n}", String(comments.length))}
        </h2>
        <ul className="flex flex-col divide-y divide-line">
          {comments.map((c) => (
            <li key={c.id} className="py-3">
              <p className="text-sm font-bold">{c.username ?? dict.torrent.anonymous}</p>
              <p className="mt-1 text-sm whitespace-pre-wrap">{c.body}</p>
              <p className="mt-1 text-[11px] text-sub">
                {new Date(c.created_at).toLocaleString(dateLocale(locale))}
              </p>
            </li>
          ))}
          {comments.length === 0 && (
            <li className="py-4 text-center text-sub">{dict.torrent.noComments}</li>
          )}
        </ul>
      </section>
    </article>
  );
}
