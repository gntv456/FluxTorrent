"use client";

import { BTN_SM_GHOST, INPUT_CARD, INPUT_GROW } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 保种认领面板（从 admin-p2-tools.tsx 按域拆出，300 门禁）：
 *  认领记录浏览（状态/关键词筛选 + 分页）与移出保种区。 */

interface ClaimRow {
  torrent_id: number;
  torrent_name: string | null;
  seeders: number;
  claimed_by: string | null;
  claimed_at: string | null;
  seed_time_delta: number;
  uploaded_delta: number;
  exited_at: string | null;
  exit_reason: string | null;
}

function fmtDeltaSec(sec: number, unit: string): string {
  if (sec <= 0) return `0 ${unit}`;
  return `${(sec / 3600).toFixed(1)} ${unit}`;
}

function fmtBytes(n: number): string {
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

export function Claims({ flash }: { flash: (m: string) => void }) {
  const { dict, locale } = useI18n();
  const at = dict.adminClaims;
  const c = dict.common;
  const [rows, setRows] = useState<ClaimRow[]>([]);
  const [state, setState] = useState("all");
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);

  const load = useCallback(async () => {
    const params = new URLSearchParams();
    params.set("state", state);
    if (q.trim()) params.set("q", q.trim());
    params.set("page", String(page));
    try {
      const r = await api.get<{ rows: ClaimRow[]; total: number }>(
        `/api/v1/admin/claims?${params.toString()}`,
      );
      setRows(r.rows);
      setTotal(r.total);
    } catch {
      setRows([]);
    }
  }, [state, q, page]);
  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2">
        <select
          value={state}
          onChange={(e) => {
            setState(e.target.value);
            setPage(1);
          }}
          className={INPUT_CARD}
        >
          <option value="all">{at.optAll}</option>
          <option value="active">{at.optActive}</option>
          <option value="unclaimed">{at.optUnclaimed}</option>
          <option value="exited">{at.optExited}</option>
        </select>
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={at.qPh}
          className={INPUT_GROW}
        />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{at.thTorrent}</td>
            <td className="colhead">{at.thSeeders}</td>
            <td className="colhead">{at.thClaimedBy}</td>
            <td className="colhead">{at.thClaimedAt}</td>
            <td className="colhead">{at.thSeedSince}</td>
            <td className="colhead">{at.thUploadSince}</td>
            <td className="colhead">{at.thStatus}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.torrent_id}>
              <td className="max-w-[200px] truncate">
                <a className="text-link" href={`/torrents?id=${r.torrent_id}`}>
                  {r.torrent_name ?? `#${r.torrent_id}`}
                </a>
              </td>
              <td>{r.seeders}</td>
              <td>{r.claimed_by ?? "—"}</td>
              <td className="text-xs">
                {r.claimed_at
                  ? new Date(r.claimed_at).toLocaleString(dateLocale(locale))
                  : "—"}
              </td>
              <td>
                {r.claimed_by
                  ? fmtDeltaSec(r.seed_time_delta, at.hoursUnit)
                  : "—"}
              </td>
              <td>{r.claimed_by ? fmtBytes(r.uploaded_delta) : "—"}</td>
              <td className="text-xs">
                {r.exited_at
                  ? fmt(at.exitedWith, { reason: r.exit_reason ?? "manual" })
                  : r.claimed_by
                    ? at.claiming
                    : at.waiting}
              </td>
              <td className="text-right">
                {!r.exited_at && (
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={async () => {
                      try {
                        await api.post("/api/v1/admin/claims/release", {
                          torrent_id: r.torrent_id,
                        });
                        flash(fmt(at.released, { id: r.torrent_id }));
                        load();
                      } catch (e) {
                        flash(e instanceof ApiError ? e.message : at.opFail);
                      }
                    }}
                  >
                    {at.release}
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-between text-sm text-sub">
        <span>{fmt(c.totalItems, { n: total })}</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className={BTN_SM_GHOST}
          >
            {c.prevPage}
          </button>
          <span>{fmt(c.pageX, { n: page })}</span>
          <button
            disabled={rows.length < 20}
            onClick={() => setPage(page + 1)}
            className={BTN_SM_GHOST}
          >
            {c.nextPage}
          </button>
        </div>
      </div>
    </div>
  );
}
