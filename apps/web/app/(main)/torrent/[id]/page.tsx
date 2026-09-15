import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { categoryColor, editionName, formatBytes, promotionBadge } from "@/lib/format";
import { hasBBCode, renderBBCode } from "@/lib/bbcode";
import { DownloadButton } from "@/components/download-button";
import { TorrentSocial } from "@/components/torrent-social";
import { TorrentManage } from "@/components/torrent-manage";
import { SnatchList } from "@/components/snatch-list";
import { TorrentTags } from "@/components/torrent-tags";
import { ResurrectButton } from "@/components/resurrect-button";
import { WishlistButton } from "@/components/wishlist";
import { CommentDeleteButton } from "@/components/comment-delete-button";
import { GroupSubscribeButton } from "@/components/group-subscribe-button";
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
  price: number;
  purchased: boolean;
  is_owner: boolean;
  /** 动态属性（0085/0087）：kind → { dict_id, name, label, sort } */
  sections?: Record<string, { dict_id: number; name: string; label: string; sort: number }>;
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

/** 聚合组（0069）：同一资源的多个版本 */
interface GroupInfo {
  group: { id: number; name: string; descr: string | null } | null;
  items: Array<{
    id: number;
    name: string;
    small_descr: string | null;
    size: number;
    seeders: number;
    leechers: number;
    times_completed: number;
    official: boolean;
    current: boolean;
  }>;
}

/** 简介 markdown-lite 渲染：标题/列表/段落（descr 为简单 markdown 文本，无需完整 parser）
 *  含 BBCode 标签时走 BBCode 渲染器（NP 口径：颜色/字体/字号/引用/代码/图片/链接…）
 *  超长简介默认折叠（馒头/阳光口径：展开按钮在底部） */
function Descr({ text }: { text: string }) {
  if (hasBBCode(text)) {
    return <div className="whitespace-pre-wrap text-sm leading-relaxed">{renderBBCode(text)}</div>;
  }
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

/** 规格网格单元（阳光站口径：数值 + 下方灰字说明） */
function Spec({ value, label, num }: { value: React.ReactNode; label: string; num?: boolean }) {
  return (
    <div className="td-spec">
      <b className={num ? "num" : undefined}>{value}</b>
      <span>{label}</span>
    </div>
  );
}

/** 折叠分区（馒头口径：默认收起，summary 带计数） */
function Fold({
  title,
  count,
  children,
  open,
}: {
  title: string;
  count?: number;
  children: React.ReactNode;
  open?: boolean;
}) {
  return (
    <details className="td-fold" open={open}>
      <summary>
        <h2>{title}</h2>
        {count !== undefined && <span className="td-fold__count num">{count}</span>}
        <span className="td-fold__chev" aria-hidden />
      </summary>
      <div className="td-fold__body">{children}</div>
    </details>
  );
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
  const [ext, files, thanks, comments, group] = await Promise.all([
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
    // 聚合组（0069）：无组/接口失败时静默降级
    api
      .get<GroupInfo>(`/api/v1/torrents/${encodeURIComponent(tid)}/group`)
      .catch(() => null),
  ]);
  const nfo = await api
    .get<{ nfo: string | null }>(`/api/v1/torrents/${encodeURIComponent(tid)}/nfo`)
    .catch(() => ({ nfo: null }));

  const { dict, locale } = await getDict();
  const d = dict.tdetail;
  const promo = promotionBadge(t.promotion);
  const edition = editionName(t.edition_id);
  const grade = t.grade_id !== null ? dict.torrents.grades[t.grade_id + 1] : undefined;
  // 0087：介质列可空（新数据在 sections），老数据仍从字典翻译
  const medium =
    t.medium_id !== null ? (dict.torrents.media[t.medium_id] ?? undefined) : undefined;
  const category = dict.torrents.categories[t.category_id] ?? String(t.category_id);
  // 动态属性（0085/0087）：sections 带维度显示名与排序，直接铺进规格网格
  const secEntries = Object.entries(ext?.sections ?? {})
    .map(([kind, v]) => ({ kind, ...v }))
    .filter((v) => v.name && !(v.kind === "media" && medium));
  const sectionOf = (kind: string) =>
    secEntries.find((v) => v.kind === kind)?.name;
  // 促销剩余时间（好学站「x天x时」口径）
  const left = t.promotion_ends_at
    ? (() => {
        const ms = new Date(t.promotion_ends_at).getTime() - Date.now();
        if (ms <= 0) return null;
        const dd = Math.floor(ms / 86400000);
        const hh = Math.floor((ms % 86400000) / 3600000);
        return `${dd}${d?.dayUnit ?? "天"}${hh}${d?.hourUnit ?? "时"}`;
      })()
    : null;
  // 副题链（与资源库列表同口径：学段 · 媒介 · 版本）
  const subtitleChain = [grade, medium, edition].filter(Boolean).join(" · ");
  // 相对时间（馒头口径：发布于 x 天前；完整时间放 title）
  const relTime = (iso: string) => {
    const ms = Date.now() - new Date(iso).getTime();
    if (ms < 3600000) return d?.justNow ?? "刚刚";
    if (ms < 86400000) return `${Math.floor(ms / 3600000)} ${d?.hourUnit ?? "时"}${d?.agoUnit ?? "前"}`;
    return `${Math.floor(ms / 86400000)} ${d?.dayUnit ?? "天"}${d?.agoUnit ?? "前"}`;
  };

  return (
    <article className="td-page">
      {/* ===== 面包屑（阳光口径：资源库 / 分类 · ID） ===== */}
      <nav className="td-crumbs" aria-label="breadcrumb">
        <a href="/torrents">{dict.nav.library}</a>
        <span aria-hidden>›</span>
        <span>{category}</span>
        <span className="td-crumbs__id">#{t.id}</span>
      </nav>

      {/* ===== 海报头（馒头/阳光口径：左海报 + 右主信息 + 操作行） ===== */}
      <section className="td-head nexus-detail">
        <div className="td-head__poster">
          <span className="td-head__cat" style={{ background: categoryColor(t.category_id) }}>
            {category}
          </span>
          {t.poster ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={t.poster} alt="" className="td-head__img" />
          ) : (
            <span
              className="td-head__img td-head__img--fallback"
              style={{ background: categoryColor(t.category_id) }}
            >
              🎬
            </span>
          )}
        </div>

        <div className="td-head__main">
          <h1 className="td-head__name font-display">{t.name}</h1>
          {(t.small_descr || subtitleChain) && (
            <p className="td-head__sub">
              {subtitleChain && <span className="td-head__chain">{subtitleChain}</span>}
              {subtitleChain && t.small_descr && <span aria-hidden> · </span>}
              {t.small_descr}
            </p>
          )}

          <div className="td-head__badges">
            {t.sticky && <span className="sticker bg-sun text-ink">{dict.torrent.sticky}</span>}
            {promo && <span className={`sticker ${promo.className}`}>{dict.promotion[promo.key]}</span>}
            {left && (
              <span className="td-head__left">
                {d?.promoLeft ?? "剩余"} {left}
              </span>
            )}
            {t.official && <span className="sticker bg-indigo text-white">{dict.torrent.official}</span>}
            {t.anonymous && <span className="sticker td-anon-sticker">{dict.torrent.anonymous}</span>}
          </div>

          <div className="td-head__meta">
            <span>
              {dict.torrent.uploader}：
              <b>{t.anonymous ? dict.torrent.anonymous : (t.owner_name ?? "—")}</b>
            </span>
            <span title={new Date(t.created_at).toLocaleString(dateLocale(locale))}>
              {dict.torrent.uploadedAt}：<b>{relTime(t.created_at)}</b>
            </span>
            <span title={new Date(ext?.last_action ?? t.created_at).toLocaleString(dateLocale(locale))}>
              {d?.lastActivity ?? "最近活动"}：<b>{relTime(ext?.last_action ?? t.created_at)}</b>
            </span>
          </div>

          {/* 操作行（馒头口径：主下载 + 次级动作横排） */}
          <div className="td-head__actions">
            <DownloadButton
              torrentId={t.id}
              name={t.name}
              price={ext?.price}
              purchased={ext?.purchased}
              isOwner={ext?.is_owner}
            />
            <TorrentSocial torrentId={t.id} />
            <WishlistButton keyword={t.name} />
            {t.seeders === 0 && (
              <ResurrectButton torrentId={t.id} name={t.name} />
            )}
          </div>
        </div>

        {/* 状态卡（阳光站口径：做种/下载/完成三色统计） */}
        <aside className="td-stats">
          <div className="td-stats__row td-stats__row--seed">
            <span>{dict.torrent.seeding}</span>
            <b className="num">{t.seeders}</b>
          </div>
          <div className="td-stats__row td-stats__row--leech">
            <span>{dict.torrent.leeching}</span>
            <b className="num">{t.leechers}</b>
          </div>
          <div className="td-stats__row td-stats__row--done">
            <span>{dict.torrent.completed}</span>
            <b className="num">{t.times_completed}</b>
          </div>
          <div className="td-stats__row">
            <span>{d?.thanksCountLabel ?? dict.torrent.thanksCount}</span>
            <b className="num">{ext?.thanks_count ?? 0}</b>
          </div>
          <div className="td-stats__row">
            <span>{d?.heatViews ?? "查看"}</span>
            <b className="num">{ext?.views ?? "—"}</b>
          </div>
          {/* 健康度（NP 口径：做种=0 视为死种） */}
          <p
            className={`td-stats__health ${
              t.seeders === 0 ? "td-stats__health--dead" : "td-stats__health--alive"
            }`}
          >
            {t.seeders === 0 ? (d?.healthDead ?? "💀 无种，等待抢救") : (d?.healthAlive ?? "🌱 可下载")}
          </p>
        </aside>
      </section>

      {/* ===== 规格网格（阳光站口径：数值 + 灰字说明，紧凑三列） ===== */}
      <section className="td-specs nexus-detail">
        <Spec value={formatBytes(t.size)} label={dict.torrent.size} num />
        <Spec value={ext?.numfiles ?? "—"} label={dict.torrent.numFiles} num />
        <Spec value={category} label={dict.torrent.category} />
        {/* 介质/学段/版本：老数据走列翻译，新数据（列可空）走 sections（0087） */}
        {(medium || sectionOf("media")) && (
          <Spec value={medium ?? sectionOf("media")} label={dict.torrent.medium} />
        )}
        {(grade || sectionOf("grades")) && (
          <Spec value={grade ?? sectionOf("grades")} label={dict.torrent.grade} />
        )}
        {(edition || sectionOf("editions")) && (
          <Spec value={edition ?? sectionOf("editions")} label={dict.torrent.edition} />
        )}
        {secEntries
          .filter((v) => !["media", "grades", "editions"].includes(v.kind))
          .map((v) => (
            <Spec key={v.kind} value={v.name} label={v.label} />
          ))}
        {t.rating && <Spec value={t.rating} label={d?.ratingLabel ?? "评分"} num />}
        <Spec
          value={
            <span className="break-all text-[11px] font-normal text-sub">{t.info_hash.trim()}</span>
          }
          label={dict.torrent.infoHash}
        />
      </section>

      {/* ===== 标签 + 操作（阳光口径：标签行右侧放编辑/删除等低频操作） ===== */}
      <section className="td-tags nexus-detail">
        <h2 className="td-sec-title">{dict.torrTags2?.title ?? "标签"}</h2>
        <div className="td-tags__body">
          <TorrentTags torrentId={t.id} />
          <div className="td-tags__manage">
            <TorrentManage
              torrentId={t.id}
              name={t.name}
              smallDescr={t.small_descr}
              descr={ext?.descr ?? null}
              anonymous={t.anonymous}
              seeders={t.seeders}
            />
          </div>
        </div>
      </section>

      {/* ===== 简介（默认展开；其余折叠分区默认收起） ===== */}
      {ext?.descr && (
        <section className="td-descr nexus-detail">
          <h2 className="td-sec-title">{dict.torrent.descrTitle}</h2>
          <div className="td-descr__body">
            <Descr text={ext.descr} />
          </div>
        </section>
      )}

      {/* ===== NFO ===== */}
      {nfo.nfo && (
        <Fold title="NFO">
          <pre className="td-nfo">{nfo.nfo}</pre>
        </Fold>
      )}

      {/* ===== 同组版本（0069 聚合组：同一资源的多个年份/版本/清晰度） ===== */}
      {group?.group && group.items.length > 1 && (
        <Fold title={`${d?.groupTitle ?? "同组版本"}：${group.group.name}`} count={group.items.length}>
          {/* 组订阅（0075）：新版本过审时通知 */}
          <div className="mb-2 flex justify-end">
            <GroupSubscribeButton groupId={group.group.id} />
          </div>
          <table className="td-files">
            <tbody>
              {group.items.map((g) => (
                <tr key={g.id} className={g.current ? "font-bold" : undefined}>
                  <td className="min-w-0 truncate">
                    {g.current ? (
                      <span title={g.name}>{g.name}</span>
                    ) : (
                      <a href={`/torrent/${g.id}`} className="hover:underline">
                        {g.name}
                      </a>
                    )}
                    {g.official && <span className="sticker bg-indigo text-white">{dict.torrent.official}</span>}
                  </td>
                  <td className="num shrink-0 text-right text-sub">
                    {g.current ? (
                      d?.current ?? "当前"
                    ) : (
                      <>
                        <span className="text-green">{g.seeders}</span> · {formatBytes(g.size)}
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Fold>
      )}

      {/* ===== 文件列表 ===== */}
      {files.length > 0 && (
        <Fold title={dict.torrent.filesTitle} count={files.length}>
          <table className="td-files">
            <tbody>
              {files.map((f) => (
                <tr key={f.file_index}>
                  <td className="min-w-0 truncate">{f.path}</td>
                  <td className="num shrink-0 text-right text-sub">{formatBytes(f.size)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Fold>
      )}

      {/* ===== 下载/做种记录 ===== */}
      <Fold title={dict.snatches2?.title ?? "下载记录"} open>
        <SnatchList torrentId={t.id} />
      </Fold>

      {/* ===== 感谢者（馒头口径：头像占位 + 用户名） ===== */}
      {thanks.length > 0 && (
        <Fold title={dict.torrent.thankersTitle} count={ext?.thanks_count ?? thanks.length}>
          <p className="flex flex-wrap gap-2">
            {thanks.map((th, i) => (
              <span key={i} className="td-thanker" title={new Date(th.created_at).toLocaleString(dateLocale(locale))}>
                <span className="td-thanker__avatar" aria-hidden>
                  {(th.username ?? "?").slice(0, 1).toUpperCase()}
                </span>
                {th.username ?? dict.torrent.anonymous}
              </span>
            ))}
          </p>
        </Fold>
      )}

      {/* ===== 评论区 ===== */}
      <section className="td-comments nexus-detail">
        <h2 className="td-sec-title">{dict.torrent.commentsTitle.replace("{n}", String(comments.length))}</h2>
        {comments.map((c) => (
          <div key={c.id} className="td-comment">
            <div className="td-comment__side">
              <span className="td-comment__avatar" aria-hidden>
                {(c.username ?? "?").slice(0, 1).toUpperCase()}
              </span>
              <p className="text-sm font-bold">{c.username ?? dict.torrent.anonymous}</p>
              <p className="mt-1 text-[11px] text-sub" title={new Date(c.created_at).toLocaleString(dateLocale(locale))}>
                {relTime(c.created_at)}
              </p>
              {/* staff 删评：按钮常显，无权限由后端 403 兜底 */}
              <CommentDeleteButton torrentId={t.id} commentId={c.id} />
            </div>
            <p className="td-comment__body">{c.body}</p>
          </div>
        ))}
        {comments.length === 0 && (
          <p className="py-4 text-center text-sub">{dict.torrent.noComments}</p>
        )}
      </section>
    </article>
  );
}
