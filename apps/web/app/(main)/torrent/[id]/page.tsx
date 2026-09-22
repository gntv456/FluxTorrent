import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { editionName, formatBytes } from "@/lib/format";
import { TorrentManage } from "@/components/torrent-manage";
import { PromoBuyButton } from "@/components/promo-buy-button";
import { SnatchList } from "@/components/snatch-list";
import { FileTree } from "@/components/file-tree";
import { TorrentTags, type TagPayload } from "@/components/torrent-tags";
import { Descr, Spec, Fold } from "@/components/torrent-detail-parts";
import { TorrentHead } from "@/components/torrent-detail-head";
import { TorrentSubtitles } from "@/components/torrent-subtitles";
import { TorrentPeers } from "@/components/torrent-peers";
import {
  Comments,
  GroupVersions,
  Thankers,
  type GroupInfo,
  type ThankItem,
  type TorrentDetailExt,
} from "@/components/torrent-detail-blocks";
import { getDict } from "@/i18n/server";
import type {
  TorrentComment,
  TorrentListItem,
} from "@fluxtorrent/domain-types";

export const dynamic = "force-dynamic";

interface FileItem {
  file_index: number;
  path: string;
  size: number;
}

/** GET /torrents/{id}/aggregate 响应（批次三 BFF；契约收录进 domain-types 待后续批次） */
interface Aggregate {
  torrent: TorrentListItem;
  detail: TorrentDetailExt;
  files: FileItem[];
  thanks: ThankItem[];
  comments: TorrentComment[];
  nfo: string | null;
  tags: TagPayload;
}

export default async function TorrentDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const tid = Number(id);
  if (!Number.isFinite(tid)) notFound();
  // 详情页首屏（方案批次三）：一次 /aggregate 带回七块数据，RTT 6 → 1。
  // 后端对只读辅助块失败降级为空、torrent/detail 失败才 404，语义与旧版等价。
  // group（publish 模块）下一批并入；subtitles 仍由客户端组件拉取；snatches 懒加载不变。
  const enc = encodeURIComponent(tid);
  let agg: Aggregate;
  try {
    agg = await api.get<Aggregate>(`/api/v1/torrents/${enc}/aggregate`);
  } catch {
    notFound();
  }
  const t = agg.torrent;
  const ext = agg.detail;
  const files = agg.files;
  const thanks = agg.thanks;
  const comments = agg.comments;
  const nfo = { nfo: agg.nfo };
  // 聚合组（0069）：无组/接口失败时静默降级
  const group = await api
    .get<GroupInfo>(`/api/v1/torrents/${enc}/group`)
    .catch(() => null);
  // 所属合集（0157）：一种可入多个合集/系列
  const inCollections = await api
    .get<{ id: number; kind: string; name: string }[]>(
      `/api/v1/torrents/${enc}/collections`,
    )
    .catch(() => []);

  const { dict, locale } = await getDict();
  const d = dict.tdetail;
  const edition = editionName(t.edition_id);
  const grade =
    t.grade_id !== null ? dict.torrents.grades[t.grade_id + 1] : undefined;
  // 0087：介质列可空（新数据在 sections），老数据仍从字典翻译
  const medium =
    t.medium_id !== null
      ? (dict.torrents.media[t.medium_id] ?? undefined)
      : undefined;
  const category =
    dict.torrents.categories[t.category_id] ?? String(t.category_id);
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
    const h = Math.floor(ms / 3600000);
    const dd = Math.floor(ms / 86400000);
    if (ms < 86400000) return `${h} ${d?.hourUnit ?? "时"}${d?.agoUnit ?? "前"}`;
    return `${dd} ${d?.dayUnit ?? "天"}${d?.agoUnit ?? "前"}`;
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
      <TorrentHead
        t={t}
        ext={ext}
        dict={dict}
        locale={locale}
        left={left}
        subtitleChain={subtitleChain}
        relTime={relTime}
      />

      {/* ===== 规格网格（阳光站口径：数值 + 灰字说明，紧凑三列） ===== */}
      <section className="td-specs nexus-detail">
        <Spec value={formatBytes(t.size)} label={dict.torrent.size} num />
        <Spec value={ext?.numfiles ?? "—"} label={dict.torrent.numFiles} num />
        <Spec value={category} label={dict.torrent.category} />
        {/* 介质/学段/版本：老数据走列翻译，新数据（列可空）走 sections（0087） */}
        {(medium || sectionOf("media")) && (
          <Spec
            value={medium ?? sectionOf("media")}
            label={dict.torrent.medium}
          />
        )}
        {(grade || sectionOf("grades")) && (
          <Spec
            value={grade ?? sectionOf("grades")}
            label={dict.torrent.grade}
          />
        )}
        {(edition || sectionOf("editions")) && (
          <Spec
            value={edition ?? sectionOf("editions")}
            label={dict.torrent.edition}
          />
        )}
        {secEntries
          .filter((v) => !["media", "grades", "editions"].includes(v.kind))
          .map((v) => (
            <Spec key={v.kind} value={v.name} label={v.label} />
          ))}
        {t.rating && (
          <Spec value={t.rating} label={d?.ratingLabel ?? "评分"} num />
        )}
        <Spec
          value={
            <span className="break-all text-[11px] font-normal text-sub">
              {t.info_hash.trim()}
            </span>
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
            <PromoBuyButton
              torrentId={t.id}
              isOwner={Boolean(ext?.is_owner)}
            />
            <TorrentManage
              torrentId={t.id}
              name={t.name}
              smallDescr={t.small_descr}
              descr={ext?.descr ?? null}
              anonymous={t.anonymous}
              seeders={t.seeders}
              imdbId={t.imdb_id ?? null}
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

      {/* ===== MediaInfo（NP 详情页折叠块口径） ===== */}
      {ext?.mediainfo && (
        <Fold title="MediaInfo">
          <pre className="td-nfo">{ext.mediainfo}</pre>
        </Fold>
      )}

      {/* ===== NFO ===== */}
      {nfo.nfo && (
        <Fold title="NFO">
          <pre className="td-nfo">{nfo.nfo}</pre>
        </Fold>
      )}

      {/* ===== 当前在线（tracker swarm 快照：做种/下载者明细，非 staff IP 已脱敏） ===== */}
      <Fold title={dict.torrents.peersTitle}>
        <TorrentPeers torrentId={t.id} />
      </Fold>

      {/* ===== 同组版本（0069 聚合组） ===== */}
      {group?.group && group.items.length > 1 && (
        <GroupVersions group={group} dict={dict} />
      )}

      {/* ===== 所属合集（0157 阶段三聚合层） ===== */}
      {inCollections.length > 0 && (
        <section className="td-collections nexus-detail">
          <h2 className="td-sec-title">{dict.collections.inTitle}</h2>
          <div className="td-collections__list">
            {inCollections.map((c) => (
              <a
                key={c.id}
                href={`/collections/${c.id}`}
                className={`sticker ${
                  c.kind === "series"
                    ? "bg-indigo text-white"
                    : "bg-sun text-ink"
                }`}
              >
                {c.name}
              </a>
            ))}
          </div>
        </section>
      )}

      {/* ===== 文件列表 ===== */}
      {files.length > 0 && (
        <Fold title={dict.torrent.filesTitle} count={files.length}>
          <FileTree files={files} />
        </Fold>
      )}

      {/* ===== 字幕面板（0146 P0-6 + 0148 C1 同片 IMDB 合并；模块关闭时空列表） ===== */}
      <TorrentSubtitles torrentId={t.id} imdbId={t.imdb_id ?? null} />

      {/* ===== 下载/做种记录 ===== */}
      <Fold title={dict.snatches2?.title ?? "下载记录"} open>
        <SnatchList torrentId={t.id} />
      </Fold>

      {/* ===== 感谢者（馒头口径） ===== */}
      {thanks.length > 0 && (
        <Thankers
          thanks={thanks}
          count={ext?.thanks_count ?? thanks.length}
          dict={dict}
          locale={locale}
        />
      )}

      {/* ===== 评论区 ===== */}
      <Comments
        comments={comments}
        torrentId={t.id}
        dict={dict}
        locale={locale}
        relTime={relTime}
      />
    </article>
  );
}
