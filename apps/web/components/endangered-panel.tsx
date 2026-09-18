"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { formatBytes } from "@/lib/format";

/** 濒危预警雷达（/endangered，0102 社交层）。
 *
 *  GET /social/endangered/list?page&size → { enabled, endangered_seeders, health_seeders, total, list[] }
 *
 * 与 /resurrections 互补：那边是「已死」（seeders = 0，认领后做种达标发奖），
 * 这里是「濒危」（只剩个别做种者，只读展示 + 引导保种）。
 * 只读页面，不做任何写操作 —— 认领仍走现有的 /resurrections/claim。 */

type Item = {
  torrent_id: number;
  info_hash: string;
  name: string;
  seeders: number;
  leechers: number;
  times_completed: number;
  size: number;
  category_id: number;
  age_days: number;
  rescue_open: boolean;
};

type Resp = {
  enabled: boolean;
  endangered_seeders: number;
  health_seeders: number;
  page: number;
  size: number;
  total: number;
  list: Item[];
};

export function EndangeredPanel() {
  const { dict } = useI18n();
  const t = dict.endangered;
  const [data, setData] = useState<Resp | null>(null);
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setData(await api.get<Resp>("/api/v1/social/endangered/list?size=100"));
    } catch {
      setFailed(true);
      setData(null);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  /** 从预警页直接发起协作：这是「看到濒危 → 想救它」最短的路径。
   *  后端会校验「必须已下载过该资源」，未下载时给出明确提示。 */
  async function createTeam(torrentId: number) {
    if (busy !== null) return;
    setBusy(torrentId);
    setMsg(null);
    try {
      await api.post("/api/v1/social/team/create", { torrent_id: torrentId });
      setMsg(dict.teams.createOk);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.teams.createFailed);
    } finally {
      setBusy(null);
    }
  }

  // 模块未开放：静默降级（不引导、不报错）
  if (!data) {
    return (
      <section className="baozi-panel preserve-list-card">
        <div className="preserve-list-card__heading">
          <h2>⚠️ {t.title}</h2>
        </div>
        <p className="px-4 py-6 text-center text-xs text-sub">
          {failed ? t.disabled : "…"}
        </p>
      </section>
    );
  }

  if (!data.enabled) {
    return (
      <section className="baozi-panel preserve-list-card">
        <div className="preserve-list-card__heading">
          <h2>⚠️ {t.title}</h2>
        </div>
        <p className="px-4 py-6 text-center text-xs text-sub">{t.disabled}</p>
      </section>
    );
  }

  return (
    <section className="baozi-panel preserve-list-card">
      <div className="preserve-list-card__heading">
        <h2>⚠️ {t.title}</h2>
        <span className="text-xs text-sub">{fmt(t.totalLine, { n: data.total })}</span>
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
              <th className="w-20 text-center">{t.colSeeders}</th>
              <th className="w-24">{t.colSize}</th>
              <th className="w-24">{t.colAge}</th>
              <th className="w-40">{dict.torrents.colActions ?? "操作"}</th>
            </tr>
          </thead>
          <tbody>
            {data.list.map((it) => (
              <tr key={it.torrent_id}>
                <td>
                  <Link href={`/torrent/${it.torrent_id}`} className="text-link">
                    {it.name}
                  </Link>
                </td>
                <td className="num text-center font-bold text-[var(--baozi-orange-dark)]">
                  {it.seeders}
                </td>
                <td className="num text-sub">{formatBytes(it.size)}</td>
                <td className="num text-sub">{fmt(t.days, { n: Math.floor(it.age_days) })}</td>
                <td>
                  {it.rescue_open ? (
                    <span className="text-xs font-bold text-mint">{t.rescueOpen}</span>
                  ) : (
                    <div className="flex flex-col items-start gap-1">
                      <span className="text-xs text-sub">
                        {fmt(t.healthGap, { n: Math.max(data.health_seeders - it.seeders, 0) })}
                      </span>
                      <button
                        type="button"
                        disabled={busy !== null}
                        onClick={() => createTeam(it.torrent_id)}
                        className="min-h-[30px] rounded-full border border-[var(--baozi-orange-dark)] px-3 text-xs font-bold text-[var(--baozi-orange-dark)] disabled:opacity-50"
                      >
                        {busy === it.torrent_id ? "…" : t.startTeam}
                      </button>
                    </div>
                  )}
                </td>
              </tr>
            ))}
            {data.list.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
