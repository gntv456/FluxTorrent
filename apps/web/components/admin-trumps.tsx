"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface TrumpRow {
  id: number;
  torrent_id: number;
  reason: string;
  note: string;
  status: string;
  /** 建议替代的种（可空） */
  target_torrent_id: number | null;
  created_at: string;
}

/** 举报处理（0336 trumping）：版主待处理队列 + 接受/驳回。
 *
 *  0336 已交付后端三个端点，但没有管理台入口 ⇒ 版主看不到队列。
 *  接受即淘汰被举报种（后端置 `approval_status=2` + `deny_note`），
 *  驳回只改举报单状态。两条路径都留痕（handled_by / handled_at）。 */
export function AdminTrumps() {
  const { dict } = useI18n();
  const d = dict.tdetail;
  const [rows, setRows] = useState<TrumpRow[] | null>(null);
  const [busy, setBusy] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .get<{ items: TrumpRow[] }>("/api/v1/admin/trumps")
      .then((r) => setRows(r.items ?? []))
      .catch(() => setRows([]));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function act(id: number, accept: boolean) {
    setBusy(id);
    setMsg(null);
    try {
      await api.post(`/api/v1/admin/trumps/${id}/resolve`, { accept });
      setMsg(d.reportHandled);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : d.reportFail);
    } finally {
      setBusy(0);
    }
  }

  return (
    <section className="nexus-detail">
      <h2 className="mb-2 text-base font-bold text-ink">{d.reportQueue}</h2>
      {msg && <p className="mb-2 text-xs text-sub">{msg}</p>}
      {rows && rows.length === 0 && (
        <p className="text-sm text-sub">{d.reportQueueEmpty}</p>
      )}
      {rows && rows.length > 0 && (
        <div className="baozi-wide-table-scroll">
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">ID</td>
                <td className="colhead">{d.reportColTarget}</td>
                <td className="colhead">{d.reportColReason}</td>
                <td className="colhead">{d.reportColNote}</td>
                <td className="colhead">{d.reportButton}</td>
              </tr>
              {rows.map((r) => (
                <tr key={r.id}>
                  <td className="rowfollow">{r.id}</td>
                  <td className="rowfollow">
                    <a href={`/torrent/${r.torrent_id}`}>
                      #{r.torrent_id}
                    </a>
                    {r.target_torrent_id != null && (
                      <span className="text-xs text-sub">
                        {" → "}#{r.target_torrent_id}
                      </span>
                    )}
                  </td>
                  <td className="rowfollow">{r.reason}</td>
                  <td className="rowfollow">{r.note}</td>
                  <td className="rowfollow">
                    <button
                      type="button"
                      className="sticker"
                      disabled={busy === r.id}
                      onClick={() => act(r.id, true)}
                    >
                      {d.reportAccept}
                    </button>
                    <button
                      type="button"
                      className="sticker ml-1"
                      disabled={busy === r.id}
                      onClick={() => act(r.id, false)}
                    >
                      {d.reportReject}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
