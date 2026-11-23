/**
 * 详情页 SEO/OG 元数据（从 page.tsx 拆出，2026-11-23；实现独立，page.tsx
 * 只 re-export `generateMetadata`——Next 要求它从 page 导出）。
 *
 * 站型分型尾巴：标题带分类，描述带维度摘要（季/话数/作者/联赛…按站型
 * 自动生效）；og 卡只在站长开启 indexable 时发——私有站默认对爬虫关门
 * （robots 口径与 layout 全局一致），分享卡（Telegram/Discord 预览）在
 * 关闭时同样收敛，不泄漏种子标题。
 */

import type { Metadata } from "next";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
import { siteBase } from "@/lib/site-url";
import { getSiteProfile } from "@/lib/site-profile";
import type { RipLog } from "@/components/torrent-rip-logs";
import type { TagPayload } from "@/components/torrent-tags";
import type {
  ThankItem,
  TorrentDetailExt,
} from "@/components/torrent-detail-blocks";
import type {
  TorrentComment,
  TorrentListItem,
} from "@fluxtorrent/domain-types";
import type { PreserveClaimState } from "./torrent-detail-claim";

export interface FileItem {
  file_index: number;
  path: string;
  size: number;
}

/** GET /torrents/{id}/aggregate 响应（批次三 BFF；契约收录进 domain-types 待后续批次） */
export interface Aggregate {
  torrent: TorrentListItem;
  /** preserve：保种认领位，仅 viewer 态下发，不在区时缺省 */
  detail: TorrentDetailExt & { preserve?: PreserveClaimState };
  files: FileItem[];
  /** 抓轨日志清单（0312；正文按碟懒加载 /torrents/{id}/logs/{ordinal}） */
  logs: RipLog[];
  thanks: ThankItem[];
  comments: TorrentComment[];
  nfo: string | null;
  tags: TagPayload;
}

/** 维度摘要：label → 首值（sections 值对象里取 name），拼进 description */
function sectionSummary(sections: unknown): string {
  const secs = (sections ?? {}) as Record<
    string,
    { label?: string; name?: string; values?: string[] }
  >;
  return Object.values(secs)
    .map((s) => {
      const v = s.name?.trim() || s.values?.[0]?.trim() || "";
      const label = s.label?.trim() || "";
      return label && v ? `${label}: ${v}` : "";
    })
    .filter(Boolean)
    .slice(0, 4)
    .join(" · ");
}

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
  const dims = sectionSummary(agg.detail.sections);
  const title = `${t.name} | ${brand}`.slice(0, 95);
  const desc = [
    t.small_descr?.trim(),
    dims,
    `${formatBytes(t.size)} · #${t.category_id}`,
  ]
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
