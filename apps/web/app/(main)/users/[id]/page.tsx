import { notFound } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { FollowButton } from "@/components/forum-follow";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";

export const dynamic = "force-dynamic";

interface Profile {
  id: number;
  username: string;
  title: string | null;
  avatar_url: string | null;
  class_id: number;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  donor: boolean;
  created_at: string;
  last_seen_at: string | null;
  seeding: number;
  leeching: number;
  uploads: number;
  comments: number;
  medals: number;
}

interface RecentUpload {
  id: number;
  name: string;
  small_descr: string | null;
  size: number;
  created_at: string;
}

interface RecentComment {
  torrent_id: number;
  body: string;
  created_at: string;
}

interface TorrentHistRow {
  torrent_id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  seeding: boolean;
}

interface ProfileData {
  profile: Profile;
  recent_uploads: RecentUpload[];
  recent_comments: RecentComment[];
}

/** 用户公开主页（NP userdetails.php 口径）：资料卡 + 分享率 + 近期种子/评论 */
export default async function UserProfilePage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ tab?: string }>;
}) {
  const { id } = await params;
  const { tab } = await searchParams;
  const uid = Number(id);
  if (!Number.isFinite(uid)) notFound();
  // 种子历史 tab（NP userdetails Torrent History 口径）：uploads=公开发布 / seeding=当前做种
  const wantTorrents = tab === "torrents";

  let data: ProfileData;
  try {
    data = await api.get<ProfileData>(`/api/v1/users/${encodeURIComponent(uid)}`);
  } catch {
    notFound();
  }
  let torrents: { uploads: TorrentHistRow[]; seeding: TorrentHistRow[] } | null = null;
  if (wantTorrents) {
    torrents = await api
      .get<{ uploads: TorrentHistRow[]; seeding: TorrentHistRow[] }>(
        `/api/v1/users/${encodeURIComponent(uid)}/torrentlist?limit=100`,
      )
      .catch(() => null);
  }

  const { dict, locale } = await getDict();
  const t = dict.userProfile2 ?? {
    title: "用户资料",
    uploaded: "上传量",
    downloaded: "下载量",
    ratio: "分享率",
    seeding: "做种中",
    leeching: "下载中",
    uploads: "发布数",
    comments: "评论数",
    medals: "勋章数",
    joined: "注册时间",
    lastSeen: "最近活动",
    recentUploads: "近期发布",
    recentComments: "近期评论",
    torrentHistory: "种子历史",
    histUploads: "发布",
    histSeeding: "做种中",
    histPublicOnly: "仅公开展示",
    noUploads: "暂无公开种子",
    noComments: "暂无评论",
    donor: "捐赠者",
  };
  const p = data.profile;
  const ratio = p.downloaded > 0 ? (p.uploaded / p.downloaded).toFixed(2) : "∞";
  const gb = (n: number) => `${(n / 1024 ** 3).toFixed(2)} GB`;

  return (
    <div className="flex flex-col gap-4">
      {/* 资料头（头像 + 用户名 + 头衔/等级/捐赠标） */}
      <section className="nexus-detail">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td colSpan={2} className="nexus-detail__title">
                <div className="flex items-center gap-3">
                  {p.avatar_url ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={p.avatar_url}
                      alt={p.username}
                      className="h-16 w-16 rounded-[var(--r-sm)] object-cover"
                    />
                  ) : (
                    <span
                      aria-hidden
                      className="flex h-16 w-16 items-center justify-center rounded-[var(--r-sm)] bg-cloud text-2xl"
                    >
                      👤
                    </span>
                  )}
                  <div>
                    <h1 className="font-display text-xl">
                      {p.username}
                      {p.donor && (
                        <span className="ml-2 sticker bg-sun text-ink">♥ {t.donor}</span>
                      )}
                      {/* 关注（0121）：组件自拉状态，is_self 时自行隐藏 */}
                      <FollowButton targetType="user" targetId={p.id} showCount className="ml-2" />
                    </h1>
                    <p className="text-sm text-sub">
                      {p.class_name ?? `LV${p.class_id}`}
                      {p.title ? ` · ${p.title}` : ""}
                    </p>
                  </div>
                </div>
              </td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.uploaded}</td>
              <td className="num text-mint">{gb(p.uploaded)}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.downloaded}</td>
              <td className="num text-coral">{gb(p.downloaded)}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.ratio}</td>
              <td className="num font-bold">{ratio}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.seeding}</td>
              <td className="num">{p.seeding}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.leeching}</td>
              <td className="num">{p.leeching}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.uploads}</td>
              <td className="num">{p.uploads}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.comments}</td>
              <td className="num">{p.comments}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.medals}</td>
              <td className="num">{p.medals}</td>
            </tr>
            <tr>
              <td className="nexus-detail__label">{t.joined}</td>
              <td className="num">{new Date(p.created_at).toLocaleDateString(dateLocale(locale))}</td>
            </tr>
            {p.last_seen_at && (
              <tr>
                <td className="nexus-detail__label">{t.lastSeen}</td>
                <td className="num">
                  {new Date(p.last_seen_at).toLocaleString(dateLocale(locale))}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </section>

      {/* 种子历史（NP Torrent History 口径）：入口行 + 展开时 uploads/seeding 两段 */}
      <section className="nexus-detail">
        <div className="flex flex-wrap items-center justify-between gap-2 p-2">
          <Link
            href={`/users/${uid}${wantTorrents ? "" : "?tab=torrents"}`}
            className="text-sm font-bold text-sky"
          >
            {wantTorrents ? "▾ " : "▸ "}
            {t.torrentHistory}
          </Link>
          {wantTorrents && (
            <span className="text-xs text-sub">
              {t.histUploads} {torrents?.uploads.length ?? 0} · {t.histSeeding}{" "}
              {torrents?.seeding.length ?? 0}
            </span>
          )}
        </div>
        {wantTorrents && (
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table">
              <tbody>
                <tr>
                  <td className="colhead">
                    {t.histUploads}（{t.histPublicOnly}）
                  </td>
                  <td className="colhead w-28 text-right">Size</td>
                </tr>
                {(torrents?.uploads ?? []).map((u) => (
                  <tr key={`up-${u.torrent_id}`}>
                    <td className="max-w-[420px] truncate">
                      <Link href={`/torrent/${u.torrent_id}`} className="text-sky-deep hover:underline">
                        {u.name}
                      </Link>
                    </td>
                    <td className="num shrink-0 text-right text-sub">{gb(u.size)}</td>
                  </tr>
                ))}
                {(torrents?.uploads ?? []).length === 0 && (
                  <tr>
                    <td colSpan={2} className="py-4 text-center text-sub">
                      {t.noUploads}
                    </td>
                  </tr>
                )}
                <tr>
                  <td className="colhead">{t.histSeeding}</td>
                  <td className="colhead w-28 text-right">S/L</td>
                </tr>
                {(torrents?.seeding ?? []).map((u) => (
                  <tr key={`sd-${u.torrent_id}`}>
                    <td className="max-w-[420px] truncate">
                      <Link href={`/torrent/${u.torrent_id}`} className="text-sky-deep hover:underline">
                        {u.name}
                      </Link>
                    </td>
                    <td className="num shrink-0 text-right text-sub">
                      {u.seeders} / {u.leechers}
                    </td>
                  </tr>
                ))}
                {(torrents?.seeding ?? []).length === 0 && (
                  <tr>
                    <td colSpan={2} className="py-4 text-center text-sub">
                      {t.noUploads}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* 近期发布 */}
      <section className="nexus-detail">
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{t.recentUploads}</td>
              <td className="colhead w-28 text-right">Size</td>
            </tr>
          </thead>
          <tbody>
            {data.recent_uploads.map((u) => (
              <tr key={u.id}>
                <td>
                  <Link href={`/torrent/${u.id}`} className="font-bold">
                    {u.name}
                  </Link>
                  {u.small_descr && <p className="text-xs text-sub">{u.small_descr}</p>}
                </td>
                <td className="num shrink-0 text-right text-sub">{gb(u.size)}</td>
              </tr>
            ))}
            {data.recent_uploads.length === 0 && (
              <tr>
                <td colSpan={2} className="py-6 text-center text-sub">
                  {t.noUploads}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </section>

      {/* 近期评论 */}
      <section className="nexus-detail">
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{t.recentComments}</td>
            </tr>
          </thead>
          <tbody>
            {data.recent_comments.map((c, i) => (
              <tr key={i}>
                <td>
                  <Link href={`/torrent/${c.torrent_id}`} className="text-xs font-bold text-sky">
                    #{c.torrent_id}
                  </Link>
                  <p className="text-sm whitespace-pre-wrap">{c.body}</p>
                  <p className="text-[11px] text-sub">
                    {new Date(c.created_at).toLocaleString(dateLocale(locale))}
                  </p>
                </td>
              </tr>
            ))}
            {data.recent_comments.length === 0 && (
              <tr>
                <td className="py-6 text-center text-sub">{t.noComments}</td>
              </tr>
            )}
          </tbody>
        </table>
      </section>
    </div>
  );
}
