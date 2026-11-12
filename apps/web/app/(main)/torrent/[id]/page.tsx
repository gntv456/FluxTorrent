import { notFound } from "next/navigation";
import type { Metadata } from "next";
import { api, ApiError } from "@/lib/api-client";
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
import { TorrentTags, type TagPayload } from "@/components/torrent-tags";
import { TorrentManageLazy } from "@/components/torrent-detail-lazy";
import { PromoBuyButton } from "@/components/promo-buy-button";
import { Descr } from "@/components/torrent-detail-parts";
import { TorrentHead } from "@/components/torrent-detail-head";
import type { RipLog } from "@/components/torrent-rip-logs";
import { TorrentPreviews } from "@/components/torrent-previews";
import { TorrentAnchorBar } from "@/components/torrent-anchor";
import { TorrentActionBarMount } from "@/components/torrent-actionbar-mount";
import { TorrentCrossseed } from "@/components/torrent-crossseed";
import { FoldMobile } from "@/components/fold-mobile";
import { TorrentRelated } from "@/components/torrent-related";
import type {
  GroupInfo,
  ThankItem,
  TorrentDetailExt,
} from "@/components/torrent-detail-blocks";
import { anchorDict } from "@/components/torrent-detail-dict";
import { TorrentSectionTabs } from "@/components/torrent-section-tabs";
import { relTime as rel } from "@/lib/rel-time";
import { TorrentDetailSide } from "./_parts/torrent-detail-side";
import { TorrentSpecs } from "./_parts/torrent-detail-specs";
import { sectionTabs } from "./_parts/torrent-section-tabs-data";
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
  /** 抓轨日志清单（0312；正文按碟懒加载 /torrents/{id}/logs/{ordinal}） */
  logs: RipLog[];
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
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) notFound(); else throw e;
  }
  const t = agg.torrent;
  const ext = agg.detail;
  const files = agg.files;
  const thanks = agg.thanks;
  const comments = agg.comments;
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
  // 字幕面板显隐（0146 模块开关 + view_hidden 双门，锚点条同源）
  const subOn = sectOn("subtitles") && profile?.modules?.subtitles !== false;
  // MediaInfo 站型门（0184）：metaSources 含 mediainfo 才给 tab
  const miOn = (profile?.metadata_sources ?? []).includes("mediainfo");
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
  // 分类名映射（可序列化，供 Client 组件 TorrentRelated 用；函数 prop 过不了
  // RSC 边界——「Functions cannot be passed directly to Client Components」）
  const catMap = byId(editCats);
  const category = dictName(catMap, t.category_id);
  const catC = catColor(colorMap(profile?.categories ?? []), t.category_id);
  // 促销剩余时间（同类站「x天x时」口径）
  const leftMs = t.promotion_ends_at
    ? new Date(t.promotion_ends_at).getTime() - Date.now()
    : 0;
  const left =
    leftMs > 0
      ? `${Math.floor(leftMs / 86400000)}${d?.dayUnit}` +
        `${Math.floor((leftMs % 86400000) / 3600000)}${d?.hourUnit}`
      : null;
  // 相对时间（同类站通用口径）：走 lib 的纯函数，别把闭包传过客户端边界
  const relTime = (iso: string) => rel(iso, d);

  return (
    <article className="td-page">
      {/* ===== 面包屑（阳光口径：资源库 / 分类 · ID） ===== */}
      <nav className="td-crumbs" aria-label="breadcrumb">
        <a href="/torrents">{dict.nav.library}</a>
        <span aria-hidden>›</span>
        <span>{category}</span>
        <span className="td-crumbs__id">#{t.id}</span>
      </nav>

      {/* ===== 双栏骨架（设计稿 §4：head 通栏 + 左主栏 + 右信息栏） ===== */}
      <div className="td-layout">
        {/* ===== 海报头（同类站通用口径：左海报 + 右主信息 + 操作行） ===== */}
      <TorrentHead
        t={t} ext={ext} dict={dict} locale={locale} left={left}
        category={category}
        categoryColor={catC}
        relTime={relTime} ripLogs={agg.logs}
        tags={
          agg.tags.dict.length + agg.tags.mine.length > 0 ? (
            <TorrentTags torrentId={t.id} initial={agg.tags} />
          ) : undefined
        }
        manage={
          <>
            <PromoBuyButton
              torrentId={t.id}
              approved={t.approval_status === 1}
            />
            {(isStaff || !!ext?.is_owner) && (<TorrentManageLazy
              torrentId={t.id} name={t.name} smallDescr={t.small_descr}
              descr={ext?.descr ?? null} anonymous={t.anonymous}
              categoryId={t.category_id} price={ext?.price ?? 0}
              posterUrl={t.poster} mediainfo={ext?.mediainfo ?? null}
              sections={editSecVals} secKinds={editKinds} secDict={editDict}
              cats={editCats} seeders={t.seeders} imdbId={t.imdb_id ?? null}
              tagDict={agg.tags.dict} tagMine={agg.tags.mine}
              posState={ext?.pos_state ?? 0} pickType={ext?.pick_type ?? 0}
              posStateUntil={ext?.pos_state_until ?? null} isStaff={isStaff}
              canManage={isStaff || !!ext?.is_owner}
              approved={t.approval_status === 1}
              metaSources={profile?.metadata_sources} autoOpen={sp.edit === "1"}
            />
            )}
          </>
        }
      />

      {/* ===== 移动端锚点条（样图网格行，常驻吸顶；桌面返回 null，
          导航由右信息栏承担；dict 传 5 键切片 anchorDict 防 RSC 泄漏） ===== */}
      <TorrentAnchorBar
        descr={sectOn("descr") && !!ext?.descr}
        files={sectOn("files") && files.length > 0 ? files.length : null}
        subtitles={subOn}
        comments={sectOn("comments") ? comments.length : null}
        dict={anchorDict(dict)}
      />
      <div className="td-main">
      {/* ===== 规格徽章条（本地完善版：独立卡片，置主栏首位） ===== */}
      <TorrentSpecs t={t} ext={ext} category={category} dict={dict} />

      {/* 标签（0173 移入头部） */}
      {/* ===== 简介（桌面展开；移动端 M5.8 收进折叠条降噪） ===== */}
      {sectOn("descr") && ext?.descr && (
        <FoldMobile title={dict.torrent.descrTitle} id="td-sec-descr">
          <div className="td-descr__body">
            <Descr text={ext.descr} />
          </div>
        </FoldMobile>
      )}

      {/* ===== 主栏标签组（省高改版 2026-11-13）：简介之外的折叠段落收进
          一个 tab 卡片——桌面 tab 切换（未访问 tab 零请求零 DOM，与 Fold
          门卫同语义），≤768 回落平铺折叠（锚点条形态不变）。
          自隐型组件（试读/辅种：清单客户端才知道空）不进 tabs，随其后
          平铺渲染。节点数组由 _parts/torrent-section-tabs-data 服务端组装。 */}
      <TorrentSectionTabs
        navLabel={dict.torrent.anchorNav}
        items={sectionTabs({
          sectOn, subOn, miOn, dict, d, t, ext, files, agg, group,
          comments, thanks, inCollections, locale,
        })}
      />

      {/* 试读 / 试听（0337）：清单懒加载，空清单且无管理权时整段不渲染 */}
      <TorrentPreviews
        torrentId={t.id}
        canManage={isStaff || !!ext?.is_owner}
      />

      {/* 0486：辅种插件，未启用/无命中时组件自身返回 null */}
      <TorrentCrossseed torrentId={t.id} label={dict.torrent.crossseed} />

      {/* InfoHash：无感谢者时 tabs 不收（thankers 段缺席），独立成段 */}
      {sectOn("infohash") && thanks.length === 0 && (
        <p className="td-infohash">
          <b>{dict.torrent.infoHash}</b> · {t.info_hash.trim()}
        </p>
      )}
      </div>

      {/* ===== 右信息栏上段（状态/文件/元信息，随页面滚动） ===== */}
      <TorrentDetailSide
        t={t} ext={ext} files={files} category={category} staff={isStaff}
        inCollections={inCollections} dict={dict} locale={locale}
        relTime={relTime}
      />

      {/* ===== 右信息栏下段：相关种子（移动端回落到页面底部） ===== */}
      <div className="td-side-rel">
        {sectOn("related") && (
          <TorrentRelated torrentId={t.id} catMap={catMap} />
        )}
      </div>
      </div>

      {/* M3：<md 底部固定操作条（收藏/复制/下载/更多，键盘弹出自动让位） */}
      <TorrentActionBarMount
        torrentId={t.id} name={t.name} price={ext?.price}
        labels={{ fav: "☆", copy: "⧉", download: "⬇" }}
      />
    </article>
  );
}
