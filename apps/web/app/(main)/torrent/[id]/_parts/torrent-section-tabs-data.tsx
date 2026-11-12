/**
 * 主栏标签组节点组装（省高改版 2026-11-13）：简介之外的折叠段落收进
 * tab 卡片。各段内容与原平铺一一对应，外壳从 Fold 换裸节点（桌面进
 * panel，≤768 由 TorrentSectionTabs 包 FoldMobile）。显隐门（sectOn）
 * 与数据态条件原样保留；tab 序按使用频率重排（文件/字幕/评论在前）。
 *
 * 自隐型组件（清单客户端才知道空的 TorrentPreviews / 辅种命中行）
 * 不进 tabs——tab 无法在客户端自摘，留在 page 平铺渲染。
 * GroupVersions/Thankers 的裸内容在本文件镜像（torrent-detail-blocks
 * 已顶 300 行门禁加不动 bare；两处 markup 与块件同步维护）。
 */

import { Descr, MiWarn, TorrentArtifacts }
  from "@/components/torrent-detail-parts";
import { TorrentPeersLazy, FileTreeLazy }
  from "@/components/torrent-detail-lazy";
import { TorrentRipLogs, type RipLog } from "@/components/torrent-rip-logs";
import { TorrentSubtitles } from "@/components/torrent-subtitles";
import { SnatchList } from "@/components/snatch-list";
import { Comments, type GroupInfo, type ThankItem, type TorrentDetailExt }
  from "@/components/torrent-detail-blocks";
import { GroupSubscribeButton } from "@/components/group-subscribe-button";
import { Badge } from "@/components/ui/badge";
import { CollectionTagLink } from "@/components/collection-tag-link";
import { groupDict, tailDict } from "@/components/torrent-detail-dict";
import { formatBytes } from "@/lib/format";
import { dateLocale, type Locale } from "@/i18n/config";
import type { SectionTab } from "@/components/torrent-section-tabs";
import type { Dict } from "@/i18n/zh-CN";
import type { TorrentComment, TorrentListItem }
  from "@fluxtorrent/domain-types";

/** 组装参数：page.tsx 已取齐的数据与判定（本文件零请求） */
export interface TabCtx {
  sectOn: (k: string) => boolean;
  subOn: boolean;
  miOn: boolean;
  dict: Dict;
  d: Dict["tdetail"];
  t: TorrentListItem;
  ext: TorrentDetailExt | null;
  files: { file_index: number; path: string; size: number }[];
  agg: object;
  group: GroupInfo | null;
  comments: TorrentComment[];
  thanks: ThankItem[];
  inCollections: { id: number; kind: string; name: string }[];
  locale: Locale;
}

/** 服务端组装 tab 清单（无数据的段自动缺席；空清单 = 组件自隐整段） */
export function sectionTabs(ctx: TabCtx): SectionTab[] {
  const { sectOn, subOn, dict, d, t, ext, files, group } = ctx;
  const agg = ctx.agg as {
    nfo?: string | null;
    logs?: RipLog[];
    artifacts?: { id: number }[];
    artifact_children?: unknown[];
  };
  const out: SectionTab[] = [];

  // 文件（FileTree 懒壳；空清单时组件自显「清单缺失」提示，与旧折叠
  // 卡同口径不缺席；旧锚点 id=td-files 保留——侧栏「查看全部」跳 tab）
  if (sectOn("files")) {
    out.push({
      id: "td-files",
      label: dict.torrent.filesTitle,
      count: files.length || undefined,
      node: <FileTreeLazy files={files} />,
    });
  }
  // 字幕（挂载即拉清单 → tab 激活才拉；旧锚点 id 保留）
  if (subOn) {
    out.push({
      id: "td-sec-subtitles",
      label: dict.subtitles.panelTitle,
      node: (
        <TorrentSubtitles torrentId={t.id} imdbId={t.imdb_id ?? null} bare />
      ),
    });
  }
  // 评论（计数 = 真实总数 t.comments；旧锚点 id 保留）
  if (sectOn("comments")) {
    out.push({
      id: "td-sec-comments",
      label: dict.torrent.commentsTitle.replace(/\s*\(?\{n\}\)?/, ""),
      count: t.comments ?? ctx.comments.length,
      node: (
        <Comments
          comments={ctx.comments}
          total={t.comments}
          torrentId={t.id}
          dict={tailDict(dict)}
          locale={ctx.locale}
          embedded
        />
      ),
    });
  }
  // 当前在线（peers 快照懒壳：挂载即拉 API → tab 激活才拉）
  if (sectOn("peers")) {
    out.push({
      id: "td-sec-peers",
      label: dict.torrents.peersTitle,
      node: <TorrentPeersLazy torrentId={t.id} />,
    });
  }
  // 下载记录（按钮触发拉取）
  if (sectOn("snatches")) {
    out.push({
      id: "td-sec-snatches",
      label: dict.snatches2.title,
      node: <SnatchList torrentId={t.id} />,
    });
  }
  // 同组版本（裸内容镜像自 GroupVersions：表 + 订阅按钮）
  if (sectOn("group") && group?.group && group.items.length > 0) {
    out.push({
      id: "td-sec-group",
      label: d.groupTitle,
      count: group.items.length,
      node: (
        <>
          <div className="mb-2 flex justify-end">
            <GroupSubscribeButton
              groupId={group.group.id}
              initialSubscribed={group.subscribed ?? false}
            />
          </div>
          <div className="brand-wide-table-scroll">
            <table className="td-files">
              <tbody>
                {group.items.map((g) => (
                  <tr
                    key={g.id}
                    className={g.current ? "font-bold" : undefined}
                  >
                    <td className="min-w-0 truncate">
                      {g.current ? (
                        <span title={g.name}>{g.name}</span>
                      ) : (
                        <a
                          href={`/torrent/${g.id}`}
                          className="hover:underline"
                        >
                          {g.name}
                        </a>
                      )}
                      {g.official && (
                        <Badge tone="brand" variant="solid">
                          {dict.torrent.official}
                        </Badge>
                      )}
                      {tierBadge(g.tier)}
                    </td>
                    <td className="num shrink-0 text-right text-sub">
                      {g.current ? (
                        d.current
                      ) : (
                        <>
                          <span className="text-green">{g.seeders}</span> ·{" "}
                          {formatBytes(g.size)}
                        </>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      ),
    });
  }
  // 感谢者（裸内容镜像自 Thankers；InfoHash 尾行随卡，无感谢者时
  // 由 page 独立成段）
  if (sectOn("thankers") && ctx.thanks.length > 0) {
    out.push({
      id: "td-sec-thankers",
      label: dict.torrent.thankersTitle,
      count: ext?.thanks_count ?? ctx.thanks.length,
      node: (
        <>
          <p className="flex flex-wrap gap-2">
            {ctx.thanks.map((th, i) => (
              <span
                key={i}
                className="td-thanker"
                title={new Date(th.created_at).toLocaleString(
                  dateLocale(ctx.locale),
                )}
              >
                <span className="td-thanker__avatar" aria-hidden>
                  {(th.username ?? "?").slice(0, 1).toUpperCase()}
                </span>
                {th.username ?? dict.torrent.anonymous}
              </span>
            ))}
          </p>
          {sectOn("infohash") && (
            <p className="td-infohash">
              <b>{dict.torrent.infoHash}</b> · {t.info_hash.trim()}
            </p>
          )}
        </>
      ),
    });
  }
  // MediaInfo（0184 按站型 metaSources 显隐；一致性提示随正文）
  if (sectOn("mediainfo") && ext?.mediainfo && ctx.miOn) {
    out.push({
      id: "td-sec-media",
      label: "MediaInfo",
      node: (
        <>
          <MiWarn agg={ctx.agg} d={d} />
          <pre className="td-nfo">{ext.mediainfo}</pre>
        </>
      ),
    });
  }
  // NFO
  if (sectOn("nfo") && agg.nfo) {
    out.push({
      id: "td-sec-nfo",
      label: "NFO",
      node: <pre className="td-nfo">{agg.nfo}</pre>,
    });
  }
  // 抓轨日志（无日志时整段不渲染，与组件自隐同口径）
  if (sectOn("logs") && (agg.logs?.length ?? 0) > 0) {
    out.push({
      id: "td-sec-logs",
      label: d.ripLogs,
      count: agg.logs!.length,
      node: <TorrentRipLogs torrentId={t.id} logs={agg.logs!} bare />,
    });
  }
  // 合集
  if (sectOn("collections") && ctx.inCollections.length > 0) {
    out.push({
      id: "td-sec-collections",
      label: dict.collections.inTitle,
      count: ctx.inCollections.length,
      node: (
        <div className="td-collections__list">
          {ctx.inCollections.map((c) => (
            <CollectionTagLink
              key={c.id}
              href={`/collections/${c.id}`}
              name={c.name}
              series={c.kind === "series"}
            />
          ))}
        </div>
      ),
    });
  }
  // 工件（两叠都空时整段不渲染，与组件自隐同口径）
  if ((agg.artifacts?.length ?? 0) + (agg.artifact_children?.length ?? 0) > 0) {
    out.push({
      id: "td-sec-artifacts",
      label: d.artifactsTitle,
      node: <TorrentArtifacts agg={ctx.agg} d={d} bare />,
    });
  }
  return out;
}

/** 多版本分槽徽标（镜像自 torrent-detail-blocks.tierBadge：画质档是
 *  内容语义三色，不随主题换肤） */
function tierBadge(tier?: string) {
  if (!tier || tier === "Other") return null;
  const color =
    tier === "UHD"
      ? "var(--content-tier-uhd)"
      : tier === "HD"
        ? "var(--content-tier-hd)"
        : "var(--content-tier-sd)";
  return (
    <span className="sticker ml-1 text-white" style={{ background: color }}>
      {tier}
    </span>
  );
}
