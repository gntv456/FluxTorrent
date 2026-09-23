"use client";

import { BTN_SM_BOLD, BTN_SM_GHOST, INPUT_GROW } from "@/lib/ui-classes";

/**
 * 后台种子管理·记录类子表（从 components/admin-torrents.tsx 按域拆出）：
 * DenyReasons 拒绝原因字典、OpLogs 种子操作记录。
 * LoginLogs 在 ./admin-torrents-login-logs.tsx；
 * RecordQuery 在 ./admin-torrents-record-query.tsx。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { DenyReason, TorrentOpRow } from "./admin-torrents-shared";

// ============ 拒绝原因（既有） ============

export function DenyReasons({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  const [busy, setBusy] = useState(false);
  const [rows, setRows] = useState<DenyReason[]>([]);
  const [edit, setEdit] = useState<{
    id: number | null;
    reason: string;
    sort: number;
  }>({ id: null, reason: "", sort: 0 });

  const load = useCallback(async () => {
    try {
      setRows(await api.get("/api/v1/admin/deny-reasons"));
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const save = async () => {
    if (!edit.reason.trim()) return;
    setBusy(true);
    try {
      if (edit.id === null)
        await api.post("/api/v1/admin/deny-reasons", {
          reason: edit.reason,
          sort: edit.sort,
        });
      else
        await api.put(`/api/v1/admin/deny-reasons/${edit.id}`, {
          reason: edit.reason,
          sort: edit.sort,
        });
      flash(at.saved);
      setEdit({ id: null, reason: "", sort: 0 });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold text-ink">
          {edit.id === null ? at.denyNew : fmt(at.denyEdit, { id: edit.id })}
        </h2>
        <label>
          {at.fReason}
          <input
            value={edit.reason}
            onChange={(e) => setEdit({ ...edit, reason: e.target.value })}
          />
        </label>
        <label>
          {at.fSort}
          <input
            type="number"
            value={edit.sort}
            onChange={(e) => setEdit({ ...edit, sort: Number(e.target.value) })}
          />
        </label>
        <div className="flex gap-2">
          <button
            className="baozi-button"
            disabled={busy || !edit.reason.trim()}
            onClick={save}
          >
            {at.save}
          </button>
          {edit.id !== null && (
            <button
              className={BTN_SM_BOLD}
              onClick={() => setEdit({ id: null, reason: "", sort: 0 })}
            >
              {at.cancel}
            </button>
          )}
        </div>
      </section>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">{at.thSort}</td>
            <td className="colhead">{at.thReason}</td>
            <td className="colhead">{at.thEnabled}</td>
            <td className="colhead text-right">{at.thAction}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>{r.sort}</td>
              <td>{r.reason}</td>
              <td>{r.enabled ? at.yes : at.no}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEdit({ id: r.id, reason: r.reason, sort: r.sort })
                  }
                >
                  {at.edit}
                </button>
                <button
                  className="cmgmt-act"
                  onClick={async () => {
                    try {
                      await api.put(`/api/v1/admin/deny-reasons/${r.id}`, {
                        enabled: !r.enabled,
                      });
                      flash(r.enabled ? at.disabledMsg : at.enabledMsg);
                      load();
                    } catch {
                      flash(at.opFail);
                    }
                  }}
                >
                  {r.enabled ? at.disable : at.enable}
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={async () => {
                    try {
                      await api.del(`/api/v1/admin/deny-reasons/${r.id}`);
                      flash(at.deleted);
                      load();
                    } catch {
                      flash(at.delFail);
                    }
                  }}
                >
                  {at.del}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ============ 种子操作记录（既有） ============

export function OpLogs() {
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  const c = dict.common;
  const [tid, setTid] = useState("");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<{
    rows: TorrentOpRow[];
    total: number;
    page: number;
  } | null>(null);

  useEffect(() => {
    const params = new URLSearchParams();
    if (tid.trim()) params.set("torrent_id", tid.trim());
    params.set("page", String(page));
    api
      .get<{ rows: TorrentOpRow[]; total: number; page: number } | null>(
        `/api/v1/admin/torrent-ops?${params.toString()}`,
      )
      .then(setData)
      .catch(() => setData(null));
  }, [tid, page]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-2">
        <input
          value={tid}
          onChange={(e) => setTid(e.target.value)}
          placeholder={at.qTid}
          className={INPUT_GROW}
        />
      </div>
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">{at.thTorrent}</td>
            <td className="colhead">{at.thOperator}</td>
            <td className="colhead">{at.thOp}</td>
            <td className="colhead">{at.thDetail}</td>
            <td className="colhead">{at.thTime}</td>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td className="max-w-[200px] truncate text-xs">
                #{r.torrent_id} {r.torrent_name ?? ""}
              </td>
              <td>{r.operator_name ?? "—"}</td>
              <td>{r.action}</td>
              <td className="max-w-[220px] truncate text-xs">
                {r.detail ? JSON.stringify(r.detail) : "—"}
              </td>
              <td className="text-xs">
                {new Date(r.created_at).toLocaleString()}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="flex items-center justify-end gap-2 text-sm text-sub">
        <button
          disabled={page <= 1}
          onClick={() => setPage(page - 1)}
          className={BTN_SM_GHOST}
        >
          {c.prevPage}
        </button>
        <span>
          {fmt(at.pageInfo, {
            page: data?.page ?? 1,
            total: data?.total ?? 0,
          })}
        </span>
        <button
          disabled={!data || data.rows.length < 20}
          onClick={() => setPage(page + 1)}
          className={BTN_SM_GHOST}
        >
          {c.nextPage}
        </button>
      </div>
    </div>
  );
}
