"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { toast } from "@/components/ui/toast";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { formatBytes } from "@/lib/format";
import { useIsCompact } from "@/lib/hooks/use-media";
import { TorrentRowList } from "@/components/torrent-row-card";
import { EmptyState } from "@/components/ui/empty-state";
import { SkeletonRows } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import { Pager } from "@/components/ui/pager";

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
  /** 分类名 / 发布时刻（同样仅 uploads 投影） */
  category?: string | null;
  published_at?: string | null;
}

/** 信封（2026-10-17）：后端从「裸数组 + limit 截断」改成带 total 的分页
 *  信封——旧版取满 100 条只能靠一句提示语承认「后面还有」。 */
interface ListEnvelope {
  rows: SnatchRow[];
  total: number;
  page: number;
  per: number;
  /** 仅 uploads：四格统计（样图 § my-torrents 的 qcards） */
  stats?: { total: number; pending: number; approved: number; denied: number };
}

/** 每页条数（与后端 per 上界一致） */
const PER = 50;

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
  const [page, setPage] = useState(1);
  const [data, setData] = useState<ListEnvelope | null>(null);
  const [failed, setFailed] = useState(false);
  const url = (p: number) =>
    `/api/v1/me/torrentlist?kind=${kind}&page=${p}&per=${PER}`;
  const load = (p: number) =>
    api
      .get<ListEnvelope>(url(p))
      .then((d) => {
        setData(d);
        setFailed(false);
      })
      .catch(() => {
        setFailed(true);
        setData(null);
      });
  // 0288：被拒种子原本只有服务端端点（POST /torrents/{id}/resubmit），
  // 前端零接线；而详情页又进不去（被拒态不在可见性白名单）⇒ 作者无从自助。
  const [busyId, setBusyId] = useState<number | null>(null);
  async function resubmit(id: number) {
    setBusyId(id);
    try {
      await api.post(`/api/v1/torrents/${id}/resubmit`, {});
      await load(page);
    } catch (e) {
      // 状态可能已被审核员改动，保持列表原样并允许再点
      toast.error(apiErrorMessage(dict, e));
    }
    setBusyId(null);
  }

  useEffect(() => {
    setData(null);
    setPage(1);
    void load(1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind]);

  if (failed) {
    return (
      <EmptyState
        icon="seed"
        tone="warn"
        title={dict.readState.failTitle}
        desc={dict.readState.failDesc}
      />
    );
  }
  if (data === null) {
    return <SkeletonRows />;
  }
  const rows = data.rows;
  const pages = Math.max(1, Math.ceil(data.total / PER));
  const goto = (p: number) => {
    setPage(p);
    void load(p);
  };
  // 四格统计只在「我的发布」这一档有意义（待审/过审/被拒是发布态）
  const st = data.stats;
  const strip =
    kind === "uploads" && st ? (
      <div className="pgstats">
        <div>
          <div className="k">{dict.mytl.statsTotal}</div>
          <div className="v">{st.total.toLocaleString()}</div>
        </div>
        <div>
          <div className="k">{dict.mytl.statsPending}</div>
          <div className="v">{st.pending.toLocaleString()}</div>
        </div>
        <div>
          <div className="k">{dict.mytl.statsApproved}</div>
          <div className="v">{st.approved.toLocaleString()}</div>
        </div>
        <div>
          <div className="k">{dict.mytl.statsDenied}</div>
          <div className="v">{st.denied.toLocaleString()}</div>
        </div>
      </div>
    ) : null;
  if (rows.length === 0) {
    return (
      <div className="flex flex-col gap-3">
        {strip}
        <EmptyState icon="seed" title={emptyText} />
      </div>
    );
  }
  const hint =
    rows.length >= PER && page < pages ? (
      <p className="mt-2 text-xs text-sub">
        {fmt(dict.mytl.truncated, { n: page * PER })}
      </p>
    ) : null;
  // M5.2：<640 行卡片化（与 /torrents 同一组件，消除分裂）；
  // ≥md 保留表格（列降级沿用现状）
  if (compact) {
    return (
      <div className="md:hidden">
        {strip}
        <TorrentRowList
          items={rows.map((r) => ({ ...r, id: r.torrent_id }))}
        />
        {hint}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-3">
      {strip}
      <div className="brand-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{dict.mytl.colName}</td>
              {kind === "uploads" && (
                <td className="colhead w-20">{dict.mytl.colCategory}</td>
              )}
              <td className="colhead">{dict.mytl.colSize}</td>
              <td className="colhead">{dict.mytl.colSeeders}</td>
              <td className="colhead">{dict.mytl.colLeechers}</td>
              {kind === "uploads" ? (
                <td className="colhead w-24">{dict.mytl.colStatus}</td>
              ) : (
                <td className="colhead">{dict.mytl.colDone}</td>
              )}
              <td className="colhead w-28">
                {kind === "uploads"
                  ? dict.mytl.colPublished
                  : dict.mytl.colAt}
              </td>
              {kind === "uploads" && (
                <td className="colhead w-20 text-right">
                  {dict.mytl.colAction}
                </td>
              )}
            </tr>
            {rows.map((r) => (
              <tr key={r.torrent_id}>
                <td className="max-w-[420px] truncate">
                  <Link
                    href={`/torrent/${r.torrent_id}`}
                    className="text-[var(--sky-deep)] hover:underline"
                    title={r.name}
                  >
                    {r.name}
                  </Link>
                  {r.seeding && (
                    <Badge tone="success" className="ml-1">
                      {dict.mytl.badgeSeeding}
                    </Badge>
                  )}
                  {r.leeching && (
                    <Badge tone="warn" className="ml-1">
                      {dict.mytl.badgeLeeching}
                    </Badge>
                  )}
                </td>
                {kind === "uploads" && (
                  <td className="text-xs text-sub">{r.category ?? "—"}</td>
                )}
                <td className="num text-xs">{formatBytes(r.size)}</td>
                <td className="num">{r.seeders}</td>
                <td className="num">{r.leechers}</td>
                {kind === "uploads" ? (
                  <td>
                    {/* 0284 P0-1：三态徽标 + 被拒原因（title 悬停全文） */}
                    {r.approval_status === 0 && (
                      <Badge tone="brand">{dict.mytl.badgePending}</Badge>
                    )}
                    {r.approval_status === 1 && (
                      <Badge tone="success">{dict.mytl.statsApproved}</Badge>
                    )}
                    {r.approval_status === 2 && (
                      <Badge
                        tone="danger"
                        title={r.deny_reason ?? ""}
                      >
                        {dict.mytl.badgeRejected}
                        {r.deny_reason ? `：${r.deny_reason}` : ""}
                      </Badge>
                    )}
                  </td>
                ) : (
                  <td className="num text-xs">{formatBytes(r.done)}</td>
                )}
                <td className="text-xs text-sub">
                  {timeCell(r, kind, locale)}
                </td>
                {kind === "uploads" && (
                  <td className="text-right">
                    <div
                      className={
                        "flex flex-wrap items-center justify-end gap-1"
                      }
                    >
                      <Link
                        href={`/torrent/${r.torrent_id}`}
                        className="text-xs font-bold text-[var(--sky-deep)]"
                      >
                        {dict.mytl.edit}
                      </Link>
                      {r.approval_status === 2 && (
                        <button
                          onClick={() => void resubmit(r.torrent_id)}
                          disabled={busyId === r.torrent_id}
                          className={
                            "ml-1 rounded-full border border-line px-2 "
                            + "py-[1px] text-[10px] disabled:opacity-40"
                          }
                        >
                          {dict.mytl.resubmit}
                        </button>
                      )}
                    </div>
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {hint}
      {pages > 1 && (
        <Pager page={page} pages={pages} total={data.total} onStep={goto} />
      )}
    </div>
  );
}

/** 时间列：uploads 看发布时刻（torrents.created_at），下载侧看完成时刻。
 *  旧版 uploads 恒回 NULL ⇒ 整列永远是「—」，那是投影缺列不是没数据。 */
function timeCell(
  r: SnatchRow,
  kind: string,
  locale: Parameters<typeof dateLocale>[0],
) {
  const iso = kind === "uploads" ? r.published_at : r.completed_at;
  return iso
    ? new Date(iso).toLocaleDateString(dateLocale(locale))
    : "—";
}
