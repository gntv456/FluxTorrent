import { notFound } from "next/navigation";
import type { Metadata } from "next";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
import { siteBase } from "@/lib/site-url";
import {
  byId,
  catColor,
  colorMap,
  dictName,
  getSiteProfile,
  legacyDimName,
} from "@/lib/site-profile";
import { TorrentManage } from "@/components/torrent-manage";
import { PromoBuyButton } from "@/components/promo-buy-button";
import { SnatchList } from "@/components/snatch-list";
import { FileTree } from "@/components/file-tree";
import { TorrentTags, type TagPayload } from "@/components/torrent-tags";
import { Descr, Spec, Fold } from "@/components/torrent-detail-parts";
import { TorrentHead } from "@/components/torrent-detail-head";
import { TorrentPreviews } from "@/components/torrent-previews";
import { TorrentActionBarMount } from "@/components/torrent-actionbar-mount";
import { TorrentSubtitles } from "@/components/torrent-subtitles";
import { TorrentPeers } from "@/components/torrent-peers";
import { TorrentRelated } from "@/components/torrent-related";
import {
  Comments,
  GroupVersions,
  Thankers,
  type GroupInfo,
  type ThankItem,
  type TorrentDetailExt,
} from "@/components/torrent-detail-blocks";
import { TorrentDetailTail } from "./_parts/torrent-detail-tail";
import { getDict } from "@/i18n/server";
import type { SectionKindMeta } from "@/components/admin-sections-shared";
import type {
  TorrentComment,
  TorrentListItem,
} from "@fluxtorrent/domain-types";
import { isStaffClass } from "@/lib/domain/user-class";

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

/** 详情页 SEO/OG（站型分型尾巴）：标题带分类，描述带维度摘要（季/话数/
 *  作者/联赛…按站型自动生效）；og 卡只在站长开启 indexable 时发——
 *  私有站默认对爬虫关门（robots 口径与 layout 全局一致），
 *  分享卡（Telegram/Discord 预览）在关闭时同样收敛，不泄漏种子标题。 */
export async function generateMetadata({
  params,
}: {
  params: Promise<{ id: string }>;
}): Promise<Metadata> {
  const tid = Number((await params).id);
  if (!Number.isFinite(tid)) return {};
  const enc = encodeURIComponent(tid);
  const [agg, profile] = await Promise.all([
    api
      .get<Aggregate>(`/api/v1/torrents/${enc}/aggregate`)
      .catch(() => null),
    getSiteProfile().catch(() => null),
  ]);
  if (!agg) return {};
  const t = agg.torrent;
  const base = siteBase();
  const brand = profile?.brand || "";
  const indexable = profile?.seo?.indexable === true;
  // 维度摘要：label → 首值（sections 值对象里取 name），拼进 description
  const secs = (agg.detail.sections ?? {}) as Record<
    string,
    { label?: string; name?: string; values?: string[] }
  >;
  const dims = Object.values(secs)
    .map((s) => {
      const v = s.name?.trim() || s.values?.[0]?.trim() || "";
      const label = s.label?.trim() || "";
      return label && v ? `${label}: ${v}` : "";
    })
    .filter(Boolean)
    .slice(0, 4)
    .join(" · ");
  const title = `${t.name} | ${brand}`.slice(0, 95);
  const desc =
    [t.small_descr?.trim(), dims, `${formatBytes(t.size)} · #${t.category_id}`]
      .filter(Boolean)
      .join(" — ")
      .slice(0, 200);
  return {
    title,
    description: desc || undefined,
    robots: indexable
      ? { index: true, follow: true }
      : { index: false, follow: false },
    // Next 的 metadata 继承是「子页只增不删」：layout 的全局 og 卡会穿透
    // 到子页。私有站（indexable=false）须显式置空 og/twitter 各键，
    // 否则分享卡照样把种子标题泄漏出去。
    ...(indexable
      ? {
          openGraph: {
            type: "article",
            siteName: brand || undefined,
            title,
            description: desc || undefined,
            ...(base ? { url: `${base.href}torrent/${enc}` } : {}),
            images: ["/brand/og.png"],
          },
        }
      : {
          openGraph: {
            title: brand || undefined,
            description: undefined,
            images: [],
          },
          twitter: { card: "summary", title: brand || undefined, images: [] },
        }),
  };
}

export default async function TorrentDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { id } = await params;
  const sp = await searchParams;
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
  // 当前用户（0184 编辑表单推荐位 staff 判定 + 元数据源口径）
  const [me, profile, secDictAll] = await Promise.all([
    api.get<{ class_id?: number }>("/api/v1/me").catch(() => null),
    getSiteProfile().catch(() => null),
    api
      .get<
        Record<string, { id: number; name: string }[]> & {
          kinds?: SectionKindMeta[];
        }
      >("/api/v1/section-dict")
      .catch(() => null),
  ]);
  // E6 视图布局：段落隐藏门（空集=全显示；模块开关仍优先，无数据仍不渲染）
  const sectOn = (k: string) =>
    !(profile?.view_hidden?.sections ?? []).includes(k);
  const isStaff = isStaffClass(me?.class_id);
  // 分类/学段/媒介/版本词表全部取自站点档案（后端为唯一真值源）；
  // 档案不可用时 editCats 为空——下拉只剩「请选择」、名称回落 #id，
  // 不再拿另一套硬编码词表顶替（那正是分类显示 bug 的源头）
  const editCats = profile?.categories ?? [];
  const td = profile?.torrent_dicts ?? {};
  const editKinds = secDictAll?.kinds ?? [];
  const editDict: Record<string, { id: number; name: string }[]> = {};
  for (const k of editKinds) editDict[k.kind] = secDictAll?.[k.kind] ?? [];
  // 当前多维属性值（B2：detail.sections = kind → {dict_id, name, values[]}；
  // 编辑表单按 field_type 自行取初值——枚举用 dict_id、自由值/多值用 values）
  const editSecVals = Object.fromEntries(
    Object.entries(ext?.sections ?? {}).map(([k, v]) => [
      k,
      { dict_id: v.dict_id, values: v.values },
    ]),
  );
  const d = dict.tdetail;
  const category = dictName(byId(editCats), t.category_id);
  // 动态属性（0085/0087）：sections 带维度显示名与排序，直接铺进规格网格。
  // R3-三步：旧三列翻译移除——0185 已迁数据，sections 是唯一展示源。
  const secEntries = Object.entries(ext?.sections ?? {})
    .map(([kind, v]) => ({ kind, ...v }))
    .filter((v) => v.name);
  // 促销剩余时间（好学站「x天x时」口径）
  const left = t.promotion_ends_at
    ? (() => {
        const ms = new Date(t.promotion_ends_at).getTime() - Date.now();
        if (ms <= 0) return null;
        const dd = Math.floor(ms / 86400000);
        const hh = Math.floor((ms % 86400000) / 3600000);
        return `${dd}${d?.dayUnit}${hh}${d?.hourUnit}`;
      })()
    : null;
  // 副题链（R3-三步）：sections 单源——取前三维度的名称拼接（与列表同口径）
  const subtitleChain = secEntries
    .slice(0, 3)
    .map((v) => v.name)
    .join(" · ");
  // 相对时间（馒头口径：发布于 x 天前；完整时间放 title）
  const relTime = (iso: string) => {
    const ms = Date.now() - new Date(iso).getTime();
    if (ms < 3600000) return d?.justNow;
    const h = Math.floor(ms / 3600000);
    const dd = Math.floor(ms / 86400000);
    if (ms < 86400000) return `${h} ${d?.hourUnit}${d?.agoUnit}`;
    return `${dd} ${d?.dayUnit}${d?.agoUnit}`;
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
        category={category}
        categoryColor={catColor(
          colorMap(profile?.categories ?? []),
          t.category_id,
        )}
        subtitleChain={subtitleChain}
        relTime={relTime}
        tags={
          agg.tags.dict.length + agg.tags.mine.length > 0 ? (
            <TorrentTags torrentId={t.id} initial={agg.tags} />
          ) : undefined
        }
        manage={
          <>
            <PromoBuyButton torrentId={t.id} />
            <TorrentManage
              torrentId={t.id}
              name={t.name}
              smallDescr={t.small_descr}
              descr={ext?.descr ?? null}
              anonymous={t.anonymous}
              categoryId={t.category_id}
              price={ext?.price ?? 0}
              posterUrl={t.poster}
              mediainfo={ext?.mediainfo ?? null}
              sections={editSecVals}
              secKinds={editKinds}
              secDict={editDict}
              cats={editCats}
              seeders={t.seeders}
              imdbId={t.imdb_id ?? null}
              tagDict={agg.tags.dict}
              tagMine={agg.tags.mine}
              posState={ext?.pos_state ?? 0}
              posStateUntil={ext?.pos_state_until ?? null}
              pickType={ext?.pick_type ?? 0}
              isStaff={isStaff}
              metaSources={profile?.metadata_sources}
              autoOpen={sp.edit === "1"}
            />
          </>
        }
      />

      {/* ===== 规格网格（阳光站口径：数值 + 灰字说明，紧凑三列） ===== */}
      <section className="td-specs nexus-detail">
        <Spec value={formatBytes(t.size)} label={dict.torrent.size} num />
        <Spec value={ext?.numfiles ?? "—"} label={dict.torrent.numFiles} num />
        <Spec value={category} label={dict.torrent.category} />
        {/* 规格网格单源化（R3）：维度走 sections（0185 迁移；label 由
            section_kinds 下发） */}
        {secEntries.map((v) => (
          <Spec key={v.kind} value={v.name} label={v.label} />
        ))}
        {t.rating && <Spec value={t.rating} label={d?.ratingLabel} num />}
      </section>

      {/* ===== 标签（0173：移入头部，副标题与发布人之间） ===== */}

      {/* ===== 简介（默认展开；其余折叠分区默认收起） ===== */}
      {sectOn("descr") && ext?.descr && (
        <section className="td-descr nexus-detail">
          <h2 className="td-sec-title">{dict.torrent.descrTitle}</h2>
          <div className="td-descr__body">
            <Descr text={ext.descr} />
          </div>
        </section>
      )}

      {/* ===== MediaInfo（NP 详情页折叠块口径；0184 同发布页按站型 metaSources 显隐） ===== */}
      {sectOn("mediainfo") && ext?.mediainfo &&
        (profile?.metadata_sources ?? []).includes("mediainfo") && (
          <Fold title="MediaInfo">
            <pre className="td-nfo">{ext.mediainfo}</pre>
          </Fold>
        )}


      {sectOn("nfo") && nfo.nfo && (
        <Fold title="NFO">
          <pre className="td-nfo">{nfo.nfo}</pre>
        </Fold>
      )}

      {/* ===== 当前在线（tracker swarm 快照：做种/下载者明细，非 staff IP 已脱敏） ===== */}
      {sectOn("peers") && (
        <Fold title={dict.torrents.peersTitle}>
          <TorrentPeers torrentId={t.id} />
        </Fold>
      )}


      {sectOn("group") && group?.group && group.items.length > 1 && (
        <GroupVersions group={group} dict={dict} />
      )}

      {/* 试读 / 试听（0337）：清单懒加载，空清单整段不渲染 */}
      <TorrentPreviews torrentId={t.id} />


      {sectOn("collections") && inCollections.length > 0 && (
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


      {sectOn("files") && files.length > 0 && (
        <Fold title={dict.torrent.filesTitle} count={files.length}>
          <FileTree files={files} />
        </Fold>
      )}

      {/* ===== 字幕面板（0146 P0-6 + 0148 C1 同片 IMDB 合并；模块关闭不渲染） ===== */}
      {sectOn("subtitles") && profile?.modules?.subtitles !== false && (
        <TorrentSubtitles torrentId={t.id} imdbId={t.imdb_id ?? null} />
      )}

      {sectOn("snatches") && (
        <Fold title={dict.snatches2.title} open>
          <SnatchList torrentId={t.id} />
        </Fold>
      )}


      <TorrentDetailTail
        t={t}
        thanks={thanks}
        thanksCount={ext?.thanks_count ?? thanks.length}
        comments={comments}
        dict={dict}
        locale={locale}
        relTime={relTime}
        sectOn={sectOn}
      />

      {/* 相关种子（2026-10-08 P1 相似推荐）：懒加载，无数据自隐 */}
      {sectOn("related") && <TorrentRelated torrentId={t.id} />}

      {/* M3：<md 底部固定操作条（收藏/复制/下载，键盘弹出自动让位） */}
      <TorrentActionBarMount
        torrentId={t.id}
        name={t.name}
        price={ext?.price}
        labels={{ fav: "☆", copy: "⧉", download: "⬇" }}
      />
    </article>
  );
}
