import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { categoryColor, editionName, formatBytes, promotionBadge } from "@/lib/format";
import { DownloadButton } from "@/components/download-button";
import { TorrentSocial } from "@/components/torrent-social";
import { TorrentManage } from "@/components/torrent-manage";
import { SnatchList } from "@/components/snatch-list";
import { TorrentTags } from "@/components/torrent-tags";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import type { TorrentComment, TorrentListItem } from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

interface TorrentDetailExt {
  descr: string | null;
  numfiles: number;
  thanks_count: number;
  bookmark_count: number;
  last_action: string | null;
  views: number;
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
  const nfo = await api
    .get<{ nfo: string | null }>(`/api/v1/torrents/${encodeURIComponent(tid)}/nfo`)
    .catch(() => ({ nfo: null }));

  const { dict, locale } = await getDict();
  const promo = promotionBadge(t.promotion);
  const edition = editionName(t.edition_id);
  const grade =
    t.grade_id !== null ? dict.torrents.grades[t.grade_id + 1] : undefined;
  const medium = dict.torrents.media[t.medium_id] ?? String(t.medium_id);
  const category = dict.torrents.categories[t.category_id] ?? String(t.category_id);

  return (
    <article className="flex flex-col gap-4">
      {/* 信息头（NexusPHP 详情块：label/value 表格） */}
      <section className="nexus-detail">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td colSpan={2} className="nexus-detail__title">
                <div className="flex flex-wrap items-center gap-2">
                  <span
                    aria-hidden
                    className="inline-block h-[40px] w-[40px] shrink-0 rounded-[var(--r-sm)] align-middle"
                    style={{ background: categoryColor(t.category_id) }}
                  />
                  <h1 className="font-display text-xl break-all">{t.name}</h1>
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
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.download}</td>
              <td>
                <div className="flex flex-wrap items-center gap-3">
                  <DownloadButton torrentId={t.id} name={t.name} />
                  <TorrentSocial torrentId={t.id} />
                </div>
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrentManage2?.edit ?? "管理"}</td>
              <td>
                <TorrentManage
                  torrentId={t.id}
                  name={t.name}
                  smallDescr={t.small_descr}
                  descr={ext?.descr ?? null}
                  anonymous={t.anonymous}
                  seeders={t.seeders}
                />
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.size}</td>
              <td className="num">{formatBytes(t.size)}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.numFiles}</td>
              <td className="num">{ext?.numfiles ?? "—"}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.seeding}</td>
              <td className="num text-mint">{t.seeders}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.leeching}</td>
              <td className="num text-coral">{t.leechers}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.completed}</td>
              <td className="num">{t.times_completed}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrTags2?.title ?? "标签"}</td>
              <td>
                <TorrentTags torrentId={t.id} />
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.tdetail?.heatViews ?? "查看"}</td>
              <td className="num">{ext?.views ?? "—"}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.tdetail?.lastActivity ?? "最近活动"}</td>
              <td className="text-xs text-sub">
                {new Date(ext?.last_action ?? t.created_at).toLocaleString(dateLocale(locale))}
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.thanksCount}</td>
              <td className="num">{ext?.thanks_count ?? 0}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.uploader}</td>
              <td>
                {t.anonymous ? dict.torrent.anonymous : (t.owner_name ?? "—")}
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.uploadedAt}</td>
              <td className="num">
                {new Date(t.created_at).toLocaleString(dateLocale(locale))}
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.medium}</td>
              <td>{medium}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.grade}</td>
              <td>{grade ?? "—"}</td>
            </tr>
            {edition && (
              <tr>
                <td className="nexus-detail__label">{dict.torrent.edition}</td>
                <td>{edition}</td>
              </tr>
            )}
            <tr>
              <td className="nexus-detail__label">{dict.torrent.category}</td>
              <td>{category}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{dict.torrent.infoHash}</td>
              <td className="num break-all text-[11px] text-sub">
                {t.info_hash.trim()}
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      {/* 简介（descr） */}
      {ext?.descr && (
        <section className="nexus-detail">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">{dict.torrent.descrTitle}</td>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>
                  <Descr text={ext.descr} />
                </td>
              </tr>
            </tbody>
          </table>
        </section>
      )}

      {/* 文件列表（files 表有记录时展示） */}
      {files.length > 0 && (
        <section className="nexus-detail">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">{dict.torrent.filesTitle} ({files.length})</td>
                <td className="colhead w-32 text-right">{dict.torrent.size}</td>
              </tr>
            </thead>
            <tbody className="block max-h-72 overflow-y-auto">
              {files.map((f) => (
                <tr key={f.file_index}>
                  <td className="min-w-0 truncate">{f.path}</td>
                  <td className="shrink-0 text-right text-sub">{formatBytes(f.size)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {/* NFO（viewnfo.php 口径，有内容时展示） */}
      {nfo.nfo && (
        <section className="nexus-detail">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">NFO</td>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>
                  <pre className="max-h-80 overflow-auto rounded-[var(--r-sm)] bg-ink p-3 font-mono text-[11px] leading-snug text-cloud">
                    {nfo.nfo}
                  </pre>
                </td>
              </tr>
            </tbody>
          </table>
        </section>
      )}

      {/* 下载/做种记录（viewsnatches.php 口径，按需加载） */}
      <section className="nexus-detail">
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.snatches2?.title ?? "下载记录"}</td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="p-2">
                <SnatchList torrentId={t.id} />
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      {/* 感谢者（近 50 人） */}
      {thanks.length > 0 && (
        <section className="nexus-detail">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">
                  {dict.torrent.thankersTitle} ({ext?.thanks_count ?? thanks.length})
                </td>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>
                  <p className="flex flex-wrap gap-x-3 gap-y-1 text-sm">
                    {thanks.map((th, i) => (
                      <span key={i} className="font-bold text-ink">
                        {th.username ?? dict.torrent.anonymous}
                      </span>
                    ))}
                  </p>
                </td>
              </tr>
            </tbody>
          </table>
        </section>
      )}

      {/* 评论区（M07）：发表 + 列表 */}
      <section className="nexus-detail">
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">
                {dict.torrent.commentsTitle.replace("{n}", String(comments.length))}
              </td>
            </tr>
          </thead>
          <tbody>
            {comments.map((c) => (
              <tr key={c.id} className="nexus-comment">
                <td>
                  <p className="text-sm font-bold">{c.username ?? dict.torrent.anonymous}</p>
                  <p className="mt-1 text-[11px] text-sub">
                    {new Date(c.created_at).toLocaleString(dateLocale(locale))}
                  </p>
                </td>
                <td className="text-sm whitespace-pre-wrap">{c.body}</td>
              </tr>
            ))}
            {comments.length === 0 && (
              <tr>
                <td className="py-4 text-center text-sub">{dict.torrent.noComments}</td>
              </tr>
            )}
          </tbody>
        </table>
      </section>
    </article>
  );
}
