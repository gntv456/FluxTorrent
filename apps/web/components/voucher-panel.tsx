"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

type Voucher = {
  id: number;
  kind: "free" | "neutral";
  source: string;
  granted_at: string;
  expires_at: string;
  used_torrent_id: number | null;
  used_at: string | null;
};

/** 我的免费券/中性券：列表 + 对指定种子用券（0073，Gazelle FL token 口径） */
export function VoucherPanel() {
  const { dict } = useI18n();
  const [vouchers, setVouchers] = useState<Voucher[]>([]);
  const [using, setUsing] = useState<number | null>(null);
  const [torrentId, setTorrentId] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const rows = await api.get<Voucher[]>("/api/v1/me/vouchers");
      setVouchers(rows);
    } catch (err) {
      setError(apiErrorMessage(dict, err));
    }
  }, [dict]);

  useEffect(() => {
    void load();
  }, [load]);

  const open = vouchers.filter(
    (v) => !v.used_at && new Date(v.expires_at) > new Date(),
  );
  const history = vouchers.filter((v) => !open.includes(v));

  async function use(id: number) {
    const tid = Number(torrentId);
    if (!tid || tid <= 0) {
      setError(dict.vouchers.needTorrentId);
      return;
    }
    setError(null);
    try {
      await api.post("/api/v1/me/vouchers/use", {
        voucher_id: id,
        torrent_id: tid,
      });
      setMessage(dict.vouchers.used.replace("{id}", String(tid)));
      setUsing(null);
      setTorrentId("");
      await load();
    } catch (err) {
      setError(apiErrorMessage(dict, err));
    }
  }

  return (
    <section className="flex flex-col gap-3">
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">
              <h2 className="font-display">{dict.vouchers.title}</h2>
            </td>
          </tr>
        </tbody>
      </table>
      {message && <p className="text-xs text-emerald-600">{message}</p>}
      {error && <p className="text-xs text-red-500">{error}</p>}
      {open.length === 0 && (
        <p className="text-sm text-sub">{dict.vouchers.empty}</p>
      )}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {open.map((v) => (
          <div
            key={v.id}
            className="flex flex-col gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4"
          >
            <div className="flex items-center justify-between">
              <span className="font-bold">
                {v.kind === "free"
                  ? dict.vouchers.freeKind
                  : dict.vouchers.neutralKind}
              </span>
              <span className="text-xs text-sub">
                {dict.vouchers.expires}{" "}
                {new Date(v.expires_at).toLocaleDateString()}
              </span>
            </div>
            {using === v.id ? (
              <div className="flex flex-col gap-2">
                <input
                  className="rounded border border-line bg-transparent px-2 py-1 text-sm"
                  placeholder={dict.vouchers.torrentIdPlaceholder}
                  value={torrentId}
                  onChange={(e) => setTorrentId(e.target.value)}
                  inputMode="numeric"
                />
                <div className="flex gap-2">
                  <button
                    className="btn-primary text-sm"
                    onClick={() => void use(v.id)}
                  >
                    {dict.vouchers.confirm}
                  </button>
                  <button
                    className="btn-ghost text-sm"
                    onClick={() => setUsing(null)}
                  >
                    {dict.vouchers.cancel}
                  </button>
                </div>
              </div>
            ) : v.used_torrent_id ? (
              <span className="text-xs text-sub">→ #{v.used_torrent_id}</span>
            ) : (
              <button
                className="btn-primary text-sm"
                onClick={() => {
                  setUsing(v.id);
                  setMessage(null);
                  setError(null);
                }}
              >
                {dict.vouchers.use}
              </button>
            )}
          </div>
        ))}
      </div>
      {history.length > 0 && (
        <details className="rounded border border-line p-3">
          <summary className="cursor-pointer text-sm text-sub">
            {dict.vouchers.history}
          </summary>
          <ul className="mt-2 flex flex-col gap-1 text-xs text-sub">
            {history.slice(0, 30).map((v) => (
              <li key={v.id}>
                #{v.id}{" "}
                {v.kind === "free"
                  ? dict.vouchers.freeKind
                  : dict.vouchers.neutralKind}
                {v.used_torrent_id ? ` → #${v.used_torrent_id}` : ""}{" "}
                {v.used_at
                  ? `(${new Date(v.used_at).toLocaleDateString()})`
                  : ""}
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
