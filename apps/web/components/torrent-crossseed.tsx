"use client";
/**
 * 详情页「辅种」行（0486 可选插件）。
 *
 * 站长没启用、没填密钥、IYUU 不可达、这个种在别站没有——四种情况一律
 * **静默不渲染**。辅种不是详情页的必需品，任何失败都不该在用户面前变成
 * 一块报错或一条空壳。
 *
 * 数据来自 GET /api/v1/torrents/{id}/crossseed：关闭时服务端直接返回
 * enabled=false 且零出站，所以这里不会额外增加任何外部请求。
 */
import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";

type Hit = {
  sid: number;
  site: string;
  domain: string;
  torrent_id: number;
  url: string | null;
};

type Payload = { enabled: boolean; configured: boolean; sites: Hit[] };

/**
 * 站点图标（走本站代理，不直连对方站点）。
 * 拿不到就隐藏 —— 缺图不该在页面上留破图。
 */
function SiteIcon({ sid }: { sid: number }) {
  return (
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={`/api/v1/iyuu/icon/${sid}`}
      alt=""
      width={16}
      height={16}
      className="mr-1 inline-block align-[-2px]"
      onError={(e) => {
        e.currentTarget.style.display = "none";
      }}
    />
  );
}

export function TorrentCrossseed({
  torrentId,
  label,
}: {
  torrentId: number;
  label: string;
}) {
  const [sites, setSites] = useState<Hit[]>([]);

  useEffect(() => {
    let alive = true;
    api
      .get<Payload>(`/api/v1/torrents/${torrentId}/crossseed`)
      .then((r) => {
        if (alive) setSites(r.enabled ? r.sites : []);
      })
      .catch(() => {
        if (alive) setSites([]);
      });
    return () => {
      alive = false;
    };
  }, [torrentId]);

  if (sites.length === 0) return null;

  return (
    <div className="mt-3 flex flex-wrap items-center gap-2 text-xs">
      <b className="text-ink">{label}</b>
      {sites.map((s) => {
        const key = `${s.site}-${s.torrent_id}`;
        const tip = s.domain ? `${s.domain} · #${s.torrent_id}` : `#${s.torrent_id}`;
        const cls =
          "inline-flex items-center rounded-full border border-line "
          + "px-2 py-0.5 text-muted";
        // 有详情页就跳转；没有（域名不认识）就只是标签，不给假链接
        return s.url ? (
          <a
            key={key}
            href={s.url}
            target="_blank"
            rel="noopener noreferrer"
            className={`${cls} hover:border-ink hover:text-ink`}
            title={tip}
          >
            <SiteIcon sid={s.sid} />
            {s.site}
          </a>
        ) : (
          <span key={key} className={cls} title={tip}>
            <SiteIcon sid={s.sid} />
            {s.site}
          </span>
        );
      })}
    </div>
  );
}
