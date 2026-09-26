"use client";

import { useEffect, useState } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { useMediaQuery, usePosture } from "@/lib/hooks/use-media";
import { useI18n } from "@/i18n/client";

/** 种子页双栏排布（M4，方案 §6/§4.4）：md–lg1（含折叠展开一档）启用
 *  「列表-详情」双栏：左栏列表（本页 items 由父级传入渲染）+ 右栏详情
 *  内联预览。选中态存 URL ?selected=（形态切换/分享/刷新不丢）。
 *  合拢回 <md 自动退化为标准导航（本组件不渲染）。
 *  右栏内容按需拉取 aggregate（仅选中时）。
 *  折叠态（folded）经 .hinge-safe 避让铰链：双栏不跨折痕。 */

export function TorrentSplitView({
  items,
  children,
}: {
  items: { id: number; name: string; small_descr?: string | null }[];
  children: React.ReactNode;
}) {
  const renderList = children;
  const { dict } = useI18n();
  const tr = dict.tsplit;
  const sp = useSearchParams();
  const router = useRouter();
  const medium = useMediaQuery("(min-width: 768px)", false);
  const posture = usePosture();
  const selected = sp.get("selected");
  const [detail, setDetail] = useState<{
    name: string;
    seeders: number;
    leechers: number;
    size: number;
    descr: string;
  } | null>(null);

  // 选中项按需拉取（仅双栏态）
  useEffect(() => {
    if (!medium || !selected) {
      setDetail(null);
      return;
    }
    const id = Number(selected);
    const local = items.find((t) => t.id === id);
    setDetail((d) =>
      d ?? {
        name: local?.name ?? `#${id}`,
        seeders: 0,
        leechers: 0,
        size: 0,
        descr: "",
      },
    );
    let alive = true;
    import("@/lib/api-client").then(({ api }) => {
      api
        .get<{
          torrent: {
            name: string;
            seeders: number;
            leechers: number;
            size: number;
          };
          detail: { descr?: string } | null;
        }>(`/api/v1/torrents/${id}/aggregate`)
        .then((agg) => {
          if (!alive) return;
          setDetail({
            name: agg.torrent.name,
            seeders: agg.torrent.seeders,
            leechers: agg.torrent.leechers,
            size: agg.torrent.size,
            descr: (agg.detail?.descr ?? "").slice(0, 400),
          });
        })
        .catch(() => undefined);
    });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [medium, selected]);

  if (!medium) return <>{renderList}</>;

  const select = (id: number | null) => {
    const next = new URLSearchParams(sp.toString());
    if (id) next.set("selected", String(id));
    else next.delete("selected");
    router.replace(`/torrents?${next.toString()}`, { scroll: false });
  };

  return (
    <div
      className={`tsplit ${posture.folded ? "hinge-safe" : ""} ${
        posture.segments >= 2 ? `tsplit--seg${posture.segments}` : ""
      }`}
    >
      <div className="tsplit__list" onClick={(e) => {
        const a = (e.target as HTMLElement).closest("a[href^='/torrent/']");
        if (a) {
          const id = Number(
            (a as HTMLAnchorElement).getAttribute("href")?.split("/").pop(),
          );
          if (id) {
            e.preventDefault();
            select(id);
          }
        }
      }}>
        {renderList}
      </div>
      <aside className="tsplit__detail" aria-label={tr.previewAria}>
        {detail ? (
          <div className="tsplit__card">
            <h2 className="tsplit__name">{detail.name}</h2>
            <p className="tsplit__nums num">
              ↑ {detail.seeders} · ↓ {detail.leechers} ·{" "}
              {Math.round(detail.size / 1e6)} MB
            </p>
            {detail.descr && (
              <p className="tsplit__descr">{detail.descr}</p>
            )}
            <div className="tsplit__acts">
              <a
                className="tsplit__open"
                href={`/torrent/${sp.get("selected")}`}
              >
                {tr.openDetail}
              </a>
              <button type="button" onClick={() => select(null)}>
                {tr.close}
              </button>
            </div>
          </div>
        ) : (
          <div className="tsplit__empty">
            <p>{tr.emptyHint}</p>
            <p className="tsplit__hint">
              {posture.segments >= 2
                ? `viewport-segments: ${posture.segments}`
                : tr.wideMode}
            </p>
          </div>
        )}
      </aside>
    </div>
  );
}
