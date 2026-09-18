"use client";

;

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface OfferItem {
  id: number;
  username: string | null;
  torrent_id: number;
  torrent_name: string | null;
  votes: number;
  promoted: boolean;
  created_at: string;
}

/** 候选区（offers.php 口径）：投票（1 火花/票）达标转官种 */
export default function OffersPage() {
  const { dict, locale, currency } = useI18n();
  const t = dict.offers2;
  const [rows, setRows] = useState<OfferItem[]>([]);
  const [newTorrentId, setNewTorrentId] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [isStaff, setIsStaff] = useState(false);

  const load = useCallback(() => {
    api.get<OfferItem[]>("/api/v1/offers").then(setRows).catch(() => setRows([]));
  }, []);
  useEffect(load, [load]);

  useEffect(() => {
    if (!localStorage.getItem("flux.token")) return;
    api
      .get<{ class_id: number }>("/api/v1/me")
      .then((m) => setIsStaff(m.class_id >= 90))
      .catch(() => {});
  }, []);

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 2500);
  }
  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      <section className="baozi-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">{t.newOffer}</h2>
        <div className="cmgmt-form">
          <label>
            {t.torrentId}
            <input
              value={newTorrentId}
              onChange={(e) => setNewTorrentId(e.target.value.replace(/\D/g, ""))}
              placeholder="#"
            />
          </label>
          <button
            className="baozi-button self-start"
            disabled={busy || !newTorrentId}
            onClick={() =>
              guard(async () => {
                await api.post("/api/v1/offers", { torrent_id: Number(newTorrentId) });
                setNewTorrentId("");
              }, t.created)
            }
          >
            {t.btnCreate}
          </button>
          <p className="text-xs text-sub">{t.voteNote.replace("{magic}", currency)}</p>
        </div>
      </section>

      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{t.colTorrent}</td>
            <td className="colhead">{t.colUser}</td>
            <td className="colhead">{t.colVotes}</td>
            <td className="colhead">{t.colAt}</td>
            <td className="colhead text-right">{dict.cmgmt.colActions}</td>
          </tr>
          {rows.map((o) => (
            <tr key={o.id}>
              <td>
                <a href={`/torrent/${o.torrent_id}`} className="text-sky">
                  {o.torrent_name ?? `#${o.torrent_id}`}
                </a>
              </td>
              <td>{o.username ?? "—"}</td>
              <td className="num font-bold">{o.votes}</td>
              <td className="text-xs text-sub">
                {new Date(o.created_at).toLocaleDateString(dateLocale(locale))}
              </td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  disabled={busy}
                  onClick={() =>
                    guard(async () => {
                      await api.post("/api/v1/offers/vote", { offer_id: o.id });
                    }, t.voted)
                  }
                >
                  {t.btnVote}
                </button>
                {isStaff && (
                  <button
                    className="cmgmt-act cmgmt-act--ok"
                    disabled={busy}
                    onClick={() =>
                      guard(async () => {
                        await api.post("/api/v1/offers/promote", { offer_id: o.id });
                      }, t.promoted)
                    }
                  >
                    {t.btnPromote}
                  </button>
                )}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={5} className="py-6 text-center text-sub">
                {t.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
