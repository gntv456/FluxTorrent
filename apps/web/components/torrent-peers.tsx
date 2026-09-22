"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { formatBytes } from "@/lib/format";

/**
 * 当前在线 peer 面板（方案阶段二：竞品对比里唯一完全缺失的能力）。
 *
 * 数据源 = API 读 tracker 写进 Redis 的 swarm 快照（`flux:tracker:peers`），
 * 所以面板本身不接触 tracker 进程；15s 自动刷新让数字保持"活"。
 * 非 staff 的 IP 由后端脱敏（前端不做二次处理，避免两套口径）。
 */

interface PeerItem {
  peer_id: string;
  client: string;
  ip: string;
  port: number;
  seeder: boolean;
  progress: number;
  uploaded: number;
  downloaded: number;
  last_seen_secs: number;
  connectable: number;
  is_self: boolean;
}

interface PeersResp {
  available: boolean;
  masked: boolean;
  seeders: number;
  leechers: number;
  total: number;
  items: PeerItem[];
}

/** 最近活跃（秒 → 紧凑单位，无语言文案） */
function relSeen(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h`;
}

export function TorrentPeers({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const t = dict.torrents;
  const [data, setData] = useState<PeersResp | null>(null);
  const [err, setErr] = useState(false);

  const load = useCallback(async () => {
    try {
      const r = await api.get<PeersResp>(
        `/api/v1/torrents/${torrentId}/peers`,
      );
      setData(r);
      setErr(false);
    } catch {
      setErr(true);
    }
  }, [torrentId]);

  useEffect(() => {
    load();
    const timer = setInterval(load, 15000);
    return () => clearInterval(timer);
  }, [load]);

  if (err) {
    return (
      <div className="td-peers" role="status">
        <p className="py-4 text-center text-sub">
          {dict.common.loadFailed}
          <button
            type="button"
            className="ml-2 underline"
            onClick={load}
          >
            {dict.common.retry}
          </button>
        </p>
      </div>
    );
  }
  if (!data) {
    return (
      <div className="td-peers" aria-busy="true">
        <p className="py-4 text-center text-sub">…</p>
      </div>
    );
  }
  if (!data.available) {
    return (
      <div className="td-peers">
        <p className="py-3 text-center text-xs text-sub">
          {t.peersUnavailable}
        </p>
      </div>
    );
  }

  return (
    <div className="td-peers">
      <p className="td-peers__sum num">
        <span>🌱 {data.seeders}</span>
        <span>⬇️ {data.leechers}</span>
        <span>{data.total} in swarm</span>
        {data.masked && (
          <span className="text-sub">· {t.peersMasked}</span>
        )}
      </p>
      {data.items.length === 0 ? (
        <p className="py-3 text-center text-xs text-sub">{t.peersEmpty}</p>
      ) : (
        <div className="baozi-wide-table-scroll">
          <table className="nexus-table td-peers__table">
            <thead>
              <tr>
                <th className="w-32">{t.pkClient}</th>
                <th>{t.pkAddress}</th>
                <th className="w-20">{t.pkProgress}</th>
                <th className="w-40">{t.pkTransfer}</th>
                <th className="w-20">{t.pkSeen}</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((p) => (
                <tr
                  key={`${p.peer_id}-${p.port}`}
                  className={p.is_self ? "is-self" : undefined}
                >
                  <td>
                    <span className="td-peers__client">{p.client}</span>
                    {p.seeder && <span className="td-peers__tag">种</span>}
                  </td>
                  <td className="num">{p.ip}:{p.port}</td>
                  <td className="num">{p.progress.toFixed(1)}%</td>
                  <td className="num">
                    {formatBytes(p.uploaded)} / {formatBytes(p.downloaded)}
                  </td>
                  <td className="num">{relSeen(p.last_seen_secs)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
