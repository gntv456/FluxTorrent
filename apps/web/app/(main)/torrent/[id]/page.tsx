import { notFound } from "next/navigation";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
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
import { TorrentActionBarMount } from "@/components/torrent-actionbar-mount";
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
import type { SectionKindMeta } from "@/components/admin-sections-shared";
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
  const isStaff = (me?.class_id ?? 0) >= 90;
  // 分类/学段/媒介/版本词表全部取自站点档案（后端为唯一真值源）；
  // 档案不可用时 editCats 为空——下拉只剩「请选择」、名称回落 #id，
  // 不再拿另一套硬编码词表顶替（那正是分类显示 bug 的源头）
  const editCats = (profile?.categories ?? []).map((c) => ({
    id: c.id,
    name: c.name,
  }));
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
        {/* 规格网格单源化（R3-三步）：全部维度走 sections（0185 已把旧列
            存量值迁入；维度 label 由 section_kinds 下发，教育站的学段/版本
            以维度形式继续工作） */}
        {secEntries.map((v) => (
          <Spec key={v.kind} value={v.name} label={v.label} />
        ))}
        {t.rating && <Spec value={t.rating} label={d?.ratingLabel} num />}
      </section>

      {/* ===== 标签（0173：移入头部，副标题与发布人之间） ===== */}

      {/* ===== 简介（默认展开；其余折叠分区默认收起） ===== */}
      {ext?.descr && (
        <section className="td-descr nexus-detail">
          <h2 className="td-sec-title">{dict.torrent.descrTitle}</h2>
          <div className="td-descr__body">
            <Descr text={ext.descr} />
          </div>
        </section>
      )}

      {/* ===== MediaInfo（NP 详情页折叠块口径；0184 同发布页按站型 metaSources 显隐） ===== */}
      {ext?.mediainfo &&
        (profile?.metadata_sources ?? []).includes("mediainfo") && (
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

      {/* ===== 字幕面板（0146 P0-6 + 0148 C1 同片 IMDB 合并；模块关闭不渲染） ===== */}
      {profile?.modules?.subtitles !== false && (
        <TorrentSubtitles torrentId={t.id} imdbId={t.imdb_id ?? null} />
      )}
      {/* ===== 下载/做种记录 ===== */}
      <Fold title={dict.snatches2.title} open>
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

      {/* Info Hash（BT 种子唯一指纹）：不再挤占规格网格，页底小字展示 */}
      <p className="break-all border-t border-line pt-2 text-[11px] text-sub">
        <span className="font-bold">{dict.torrent.infoHash}</span> ·{" "}
        {t.info_hash.trim()}
      </p>

      {/* ===== 评论区 ===== */}
      <Comments
        comments={comments}
        torrentId={t.id}
        dict={dict}
        locale={locale}
        relTime={relTime}
      />

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
