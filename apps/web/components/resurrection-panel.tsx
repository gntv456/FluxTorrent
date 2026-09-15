"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { formatBytes } from "@/lib/format";

/** 复活任务区（0073，U3D Graveyard 口径）：
 *  GET /resurrections 可领取死种（元组 [id, name, size]）
 *  POST /resurrections/claim {torrent_id} 认领
 *  GET /resurrections/mine 我的任务（元组 [torrent_id, reward_sparks, required_hours, claimed_at, status, seeded_seconds]） */

type DeadRow = [number, string, number];
type MineRow = [number, number, number, string, string, number];

export function ResurrectionPanel() {
  const { dict, locale } = useI18n();
  const t = dict.resurrect;
  const [dead, setDead] = useState<DeadRow[] | null>(null);
  const [mine, setMine] = useState<MineRow[] | null>(null);
  const [mineOpen, setMineOpen] = useState(false);
  const [busy, setBusy] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [claimed, setClaimed] = useState<Record<number, string>>({});

  const load = useCallback(async () => {
    try {
      setDead(await api.get<DeadRow[]>("/api/v1/resurrections"));
    } catch {
      setDead([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const loadMine = useCallback(async () => {
    try {
      setMine(await api.get<MineRow[]>("/api/v1/resurrections/mine"));
    } catch {
      setMine([]);
    }
  }, []);

  async function claim(torrentId: number) {
    if (busy !== null) return;
    if (!window.confirm(t.note)) return;
    setBusy(torrentId);
    setMsg(null);
    try {
      const r = await api.post<{ torrent_id: number; required_hours: number }>(
        "/api/v1/resurrections/claim",
        { torrent_id: torrentId },
      );
      const text = t.claimed
        .replace("{hours}", String(r.required_hours))
        .replace("{reward}", t.reward);
      setClaimed((c) => ({ ...c, [torrentId]: text }));
      setMsg(text);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.claimFailed);
    } finally {
      setBusy(null);
    }
  }

  const statusLabel = (s: string) =>
    s === "open" ? t.stOpen : s === "done" ? t.stDone : t.stOther;

  return (
    <section className="baozi-panel preserve-list-card">
      <div className="preserve-list-card__heading">
        <h2>🌱 {t.title}</h2>
        <button
          type="button"
          className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold"
          onClick={() => {
            setMineOpen((v) => !v);
            if (!mineOpen) void loadMine();
          }}
        >
          {t.mine}
        </button>
      </div>
      <p className="px-4 pb-2 text-xs text-sub">{t.note}</p>
      {msg && (
        <p className="mx-4 mb-2 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink" role="status">
          {msg}
        </p>
      )}

      <div className="baozi-wide-table-scroll">
        <table className="nexus-table torrents-table">
          <thead>
            <tr>
              <th>{t.colTorrent}</th>
              <th className="w-24">{t.colSize}</th>
              <td className="colhead w-24">{dict.torrents.colActions ?? "操作"}</td>
            </tr>
          </thead>
          <tbody>
            {(dead ?? []).map(([id, name, size]) => (
              <tr key={id}>
                <td>
                  <Link href={`/torrent/${id}`} className="text-link">
                    {name}
                  </Link>
                </td>
                <td className="num text-sub">{formatBytes(size)}</td>
                <td className="text-right">
                  {claimed[id] ? (
                    <span className="text-xs font-bold text-mint">{claimed[id]}</span>
                  ) : (
                    <button
                      type="button"
                      disabled={busy !== null}
                      onClick={() => claim(id)}
                      className="min-h-[32px] rounded-full border border-[var(--baozi-orange-dark)] px-3 text-xs font-bold text-[var(--baozi-orange-dark)] disabled:opacity-50"
                    >
                      {t.claimBtn}
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {dead !== null && dead.length === 0 && (
              <tr>
                <td colSpan={3} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {mineOpen && (
        <div className="mt-3 border-t border-dashed border-line px-4 pt-3">
          <h3 className="mb-2 text-sm font-bold">{t.mine}</h3>
          <table className="nexus-table text-xs">
            <tbody>
              <tr>
                <td className="colhead">{t.colTorrent}</td>
                <td className="colhead w-28">{t.colRequired}</td>
                <td className="colhead w-28">{t.colSeeded}</td>
                <td className="colhead w-24">{t.colStatus}</td>
              </tr>
              {(mine ?? []).map((m, i) => (
                <tr key={`${m[0]}-${i}`}>
                  <td>
                    <Link href={`/torrent/${m[0]}`} className="text-link">
                      #{m[0]}
                    </Link>
                    <span className="ml-2 text-sub">
                      {new Date(m[3]).toLocaleString(dateLocale(locale))}
                    </span>
                  </td>
                  <td className="num">{fmt(t.required, { n: m[2] })}</td>
                  <td className="num">{fmt(t.seeded, { n: Math.floor(m[5] / 3600) })}</td>
                  <td>{statusLabel(m[4])}</td>
                </tr>
              ))}
              {mine !== null && mine.length === 0 && (
                <tr>
                  <td colSpan={4} className="py-4 text-center text-sub">
                    {t.mineEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
