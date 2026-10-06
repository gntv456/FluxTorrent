"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { formatBytes } from "@/lib/format";
import { useIsCompact } from "@/lib/hooks/use-media";
import { TorrentRowList } from "@/components/torrent-row-card";
import { EmptyState } from "@/components/ui/empty-state";
import { SkeletonRows } from "@/components/ui/skeleton";

interface SnatchRow {
  torrent_id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  seeding: boolean;
  leeching: boolean;
  completed_at: string | null;
  done: number;
  /** 0284 P0-1：仅 kind=uploads —— 0=待审 1=过审 2=被拒 */
  approval_status?: number;
  deny_reason?: string | null;
}

/** 我的种子列表（NP getusertorrentlist 口径）：做种中 / 已完成 / 我的发布 */
export function TorrentListClient({
  kind,
  emptyText,
}: {
  kind: "seeding" | "completed" | "uploads";
  emptyText: string;
}) {
  const { dict, locale } = useI18n();
  const compact = useIsCompact();
  const [rows, setRows] = useState<SnatchRow[] | null>(null);
  // 0288：被拒种子原本只有服务端端点（POST /torrents/{id}/resubmit），
  // 前端零接线；而详情页又进不去（被拒态不在可见性白名单）⇒ 作者无从自助。
  const [busyId, setBusyId] = useState<number | null>(null);
  async function resubmit(id: number) {
    setBusyId(id);
    try {
      await api.post(`/api/v1/torrents/${id}/resubmit`, {});
      setRows(
        await api.get<SnatchRow[]>(
          `/api/v1/me/torrentlist?kind=${kind}&limit=100`,
        ),
      );
    } catch {
      // 状态可能已被审核员改动，保持列表原样并允许再点
    }
    setBusyId(null);
  }

  useEffect(() => {
    setRows(null);
    api
      .get<SnatchRow[]>(`/api/v1/me/torrentlist?kind=${kind}&limit=100`)
      .then(setRows)
      .catch(() => setRows([]));
  }, [kind]);

  if (rows === null) {
    return <SkeletonRows />;
  }
  if (rows.length === 0) {
    return <EmptyState icon="seed" title={emptyText} />;
  }
  // M5.2：<640 行卡片化（与 /torrents 同一组件，消除分裂）；
  // ≥md 保留表格（列降级沿用现状）
  if (compact) {
    return (
      <div className="md:hidden">
        <TorrentRowList
          items={rows.map((r) => ({ ...r, id: r.torrent_id }))}
        />
      </div>
    );
  }
  return (
    <div className="baozi-wide-table-scroll">
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{dict.mytl.colName}</td>
            <td className="colhead">{dict.mytl.colSize}</td>
            <td className="colhead">{dict.mytl.colSeeders}</td>
            <td className="colhead">{dict.mytl.colLeechers}</td>
            {kind === "uploads" ? null : (
              <td className="colhead">{dict.mytl.colDone}</td>
            )}
            <td className="colhead">{dict.mytl.colAt}</td>
          </tr>
          {rows.map((r) => (
            <tr key={r.torrent_id}>
              <td className="max-w-[420px] truncate">
                <Link
                  href={`/torrent/${r.torrent_id}`}
                  className="text-sky-deep hover:underline"
                  title={r.name}
                >
                  {r.name}
                </Link>
                {r.seeding && (
                  <span className="sticker ml-1 bg-mint/30">
                    {dict.mytl.badgeSeeding}
                  </span>
                )}
                {r.leeching && (
                  <span className="sticker ml-1 bg-sun">
                    {dict.mytl.badgeLeeching}
                  </span>
                )}
                {/* 0284 P0-1：自己发布的三态徽标 + 被拒原因（title 悬停全文） */}
                {kind === "uploads" &&
                  r.approval_status === 0 && (
                    <span className="sticker ml-1 bg-sky/30">
                      {dict.mytl.badgePending}
                    </span>
                  )}
                {kind === "uploads" && r.approval_status === 2 && (
                  <>
                    <span
                      className="sticker ml-1 bg-coral/40"
                      title={r.deny_reason ?? ""}
                    >
                      {dict.mytl.badgeRejected}
                      {r.deny_reason ? `：${r.deny_reason}` : ""}
                    </span>
                    <button
                      onClick={() => resubmit(r.torrent_id)}
                      disabled={busyId === r.torrent_id}
                      className={
                      "ml-1 rounded-full border border-line px-2 py-[1px] "
                      + "text-[10px] disabled:opacity-40"
                      }
                    >
                      {dict.mytl.resubmit}
                    </button>
                  </>
                )}
              </td>
              <td className="num text-xs">{formatBytes(r.size)}</td>
              <td className="num">{r.seeders}</td>
              <td className="num">{r.leechers}</td>
              {kind === "uploads" ? null : (
                <td className="num text-xs">{formatBytes(r.done)}</td>
              )}
              <td className="text-xs text-sub">
                {r.completed_at
                  ? new Date(r.completed_at).toLocaleString(dateLocale(locale))
                  : "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
