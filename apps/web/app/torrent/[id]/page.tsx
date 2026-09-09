import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { categoryColor, editionName, formatBytes, promotionBadge } from "@/lib/format";
import { DownloadButton } from "@/components/download-button";
import { TorrentSocial } from "@/components/torrent-social";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import type { TorrentComment, TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

interface TorrentDetailExt {
  descr: string | null;
  numfiles: number;
  thanks_count: number;
  bookmark_count: number;
}

interface FileItem {
  file_index: number;
  path: string;
  size: number;
}

interface ThankItem {
  username: string | null;
  created_at: string;
}

/** 简介 markdown-lite 渲染：标题/列表/段落（descr 为简单 markdown 文本，无需完整 parser） */
function Descr({ text }: { text: string }) {
  const lines = text.split("\n");
  const out: React.ReactNode[] = [];
  let listBuf: string[] = [];
  const flushList = (key: number) => {
    if (listBuf.length) {
      out.push(
        <ul key={`ul-${key}`} className="ml-5 list-disc space-y-0.5">
          {listBuf.map((li, i) => (
            <li key={i}>{li}</li>
          ))}
        </ul>,
      );
      listBuf = [];
    }
  };
  lines.forEach((line, i) => {
    const s = line.trim();
    if (s.startsWith("- ") || s.startsWith("* ")) {
      listBuf.push(s.slice(2));
    } else {
      flushList(i);
      if (s.startsWith("#")) {
        const level = Math.min(s.match(/^#+/)?.[0].length ?? 1, 4);
        const Tag = `h${level + 2}` as "h3" | "h4" | "h5";
        out.push(
          <Tag key={i} className="mt-3 font-display text-lg first:mt-0">
            {s.replace(/^#+\s*/, "")}
          </Tag>,
        );
      } else if (s) {
        out.push(
          <p key={i} className="mt-2 first:mt-0">
            {s}
          </p>,
        );
      }
    }
  });
  flushList(lines.length);
  return <div className="text-sm leading-relaxed">{out}</div>;
}

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
  // 扩展数据与评论加载失败不阻塞详情页
  const [ext, files, thanks, comments] = await Promise.all([
    api
      .get<TorrentDetailExt>(`/api/v1/torrents/${encodeURIComponent(tid)}/detail`)
      .catch(() => null),
    api
      .get<FileItem[]>(`/api/v1/torrents/${encodeURIComponent(tid)}/files`)
      .catch(() => [] as FileItem[]),
    api
      .get<ThankItem[]>(`/api/v1/torrents/${encodeURIComponent(tid)}/thanks`)
      .catch(() => [] as ThankItem[]),
    api
      .get<TorrentComment[]>(`/api/v1/torrents/${encodeURIComponent(tid)}/comments`)
      .catch(() => [] as TorrentComment[]),
  ]);

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
                <dt className="text-sub">{dict.torrent.numFiles}</dt>
                <dd>{ext?.numfiles ?? "—"}</dd>
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
                <dt className="text-sub">{dict.torrent.thanksCount}</dt>
                <dd>{ext?.thanks_count ?? 0}</dd>
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

      {/* 简介（descr） */}
      {ext?.descr && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]">
          <h2 className="mb-2 font-display text-lg">{dict.torrent.descrTitle}</h2>
          <Descr text={ext.descr} />
        </section>
      )}

      {/* 文件列表（files 表有记录时展示） */}
      {files.length > 0 && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]">
          <h2 className="mb-2 font-display text-lg">
            {dict.torrent.filesTitle} ({files.length})
          </h2>
          <ul className="num max-h-72 divide-y divide-line overflow-y-auto text-sm">
            {files.map((f) => (
              <li key={f.file_index} className="flex items-center justify-between gap-4 py-1.5">
                <span className="min-w-0 truncate">{f.path}</span>
                <span className="shrink-0 text-sub">{formatBytes(f.size)}</span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* 感谢者（近 50 人） */}
      {thanks.length > 0 && (
        <section className="rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]">
          <h2 className="mb-2 font-display text-lg">
            {dict.torrent.thankersTitle} ({ext?.thanks_count ?? thanks.length})
          </h2>
          <p className="flex flex-wrap gap-x-3 gap-y-1 text-sm">
            {thanks.map((th, i) => (
              <span key={i} className="font-bold text-ink">
                {th.username ?? dict.torrent.anonymous}
              </span>
            ))}
          </p>
        </section>
      )}

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
