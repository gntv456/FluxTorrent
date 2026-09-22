"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 金字幕评选管理（0150 缺口4）：候选列表（双赛道）+ 生成/授金。
 *  API：GET/POST /admin/subtitles/awards/{candidates,build,grant}。 */

interface Candidate {
  id: number;
  period: string;
  subtitle_id: number;
  user_id: number;
  username: string | null;
  title: string;
  lang: string | null;
  score: number;
  rank: number;
  tier: string;
  rating: number | null;
  downloads: number;
}

export function AdminSubtitlesAwards() {
  const { dict } = useI18n();
  const t = dict.subtitlesAdmin;
  const [period, setPeriod] = useState(prevMonth());
  const [rows, setRows] = useState<Candidate[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const r = await api.get<{ period: string; items: Candidate[] }>(
        `/api/v1/admin/subtitles/awards/candidates` +
          `?period=${encodeURIComponent(period)}`,
      );
      setRows(r.items ?? []);
    } catch {
      setRows([]);
    }
  }, [period]);

  useEffect(() => {
    void load();
  }, [load]);

  async function build() {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ added: number }>(
        `/api/v1/admin/subtitles/awards/build` +
          `?period=${encodeURIComponent(period)}`,
        {},
      );
      setMsg((t.buildOk ?? "generated {n}").replace("{n}", String(r.added)));
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "failed");
    } finally {
      setBusy(false);
    }
  }

  async function grant(row: Candidate, rank: number) {
    if (
      !window.confirm(
        (t.grantConfirm ?? "grant rank {r} to {u}?")
          .replace("{r}", String(rank))
          .replace("{u}", row.username ?? `#${row.user_id}`),
      )
    )
      return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/subtitles/awards/grant", {
        period,
        grants: [{ id: row.id, rank }],
      });
      setMsg(t.grantOk ?? "granted");
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "failed");
    } finally {
      setBusy(false);
    }
  }

  const tiers: [string, Candidate[]][] = [
    ["human", (rows ?? []).filter((r) => r.tier === "human")],
    ["ai", (rows ?? []).filter((r) => r.tier === "ai")],
  ];
  return (
    <section className="nexus-detail">
      <h2 className="mb-3 text-base font-bold">{t.title}</h2>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <input
          className="w-24"
          value={period}
          placeholder="YYYY-MM"
          onChange={(e) => setPeriod(e.target.value.slice(0, 7))}
        />
        <button type="button" className="btn2" onClick={build} disabled={busy}>
          {t.build}
        </button>
        <span className="text-xs text-sub">{t.buildNote}</span>
      </div>
      {msg && <p className="mb-2 text-sm">{msg}</p>}
      {rows === null ? (
        <p className="py-6 text-center text-sub">…</p>
      ) : (
        tiers.map(([tier, list]) => (
          <div key={tier} className="mb-4">
            <h3 className="mb-1 text-sm font-bold">
              {tier === "ai" ? t.tierAI : t.tierHuman}（{list.length}）
            </h3>
            <table className="nexus-table">
              <thead>
                <tr>
                  <td className="colhead">{t.colTitle}</td>
                  <td className="colhead w-24">{t.colUser}</td>
                  <td className="colhead w-20 text-center">{t.colScore}</td>
                  <td className="colhead w-20 text-center">{t.colRank}</td>
                  <td className="colhead w-44">{t.colAction}</td>
                </tr>
              </thead>
              <tbody>
                {list.map((r) => (
                  <tr key={r.id}>
                    <td>
                      <a
                        href={`/subtitles/${r.subtitle_id}`}
                        target="_blank"
                        rel="noreferrer"
                        className="font-bold"
                      >
                        {r.title}
                      </a>
                    </td>
                    <td className="text-sub">{r.username ?? "—"}</td>
                    <td className="num text-center">{r.score}</td>
                    <td className="text-center">
                      {r.rank > 0 ? ["🥇", "🥈", "🥉"][r.rank - 1] : "—"}
                    </td>
                    <td>
                      {r.rank === 0 ? (
                        <span className="flex gap-1">
                          {[1, 2, 3].map((rk) => (
                            <button
                              key={rk}
                              type="button"
                              className="btn2"
                              disabled={busy}
                              onClick={() => void grant(r, rk)}
                            >
                              {["🥇", "🥈", "🥉"][rk - 1]}
                            </button>
                          ))}
                        </span>
                      ) : (
                        <span className="text-xs text-sub">{t.granted}</span>
                      )}
                    </td>
                  </tr>
                ))}
                {list.length === 0 && (
                  <tr>
                    <td colSpan={5} className="py-4 text-center text-sub">
                      {t.empty}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        ))
      )}
    </section>
  );
}

function prevMonth(): string {
  const now = new Date();
  const d = new Date(now.getFullYear(), now.getMonth() - 1, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}
