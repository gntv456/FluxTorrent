"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 保种协作（/teams，0102 社交层）。
 *
 *  GET  /api/v1/social/team/list  招募中的队伍（含 joined 标记）
 *  GET  /api/v1/social/team/mine  我的队伍（含成员与各自契约期增量）
 *  POST /api/v1/social/team/join|leave
 *
 * 与 /resurrections 的关系：那边是单人认领死种；这里是多人协作保种，
 * 复用同一套 resurrections 任务本体（挂 team_id），奖励按贡献分摊。 */

type TeamRow = {
  team_id: number;
  torrent_id: number;
  torrent_name: string;
  leader_name: string;
  member_count: number;
  team_size_max: number;
  deadline_at: string | null;
  seeders: number;
  joined: boolean;
};

type Member = {
  uid: number;
  username: string;
  is_leader: boolean;
  join_status: number;
  delta_seconds: number;
};

type MineRow = {
  team_id: number;
  status: number;
  team_size_max: number;
  torrent_id: number | null;
  torrent_name: string | null;
  members: Member[];
  /// 当前用户是否是队长（决定是否给出「退出」按钮）
  viewer_is_leader: boolean;
};

type ListResp = { enabled: boolean; list: TeamRow[] };
type MineResp = { enabled: boolean; list: MineRow[] };

export function TeamPanel() {
  const { dict, locale } = useI18n();
  const t = dict.teams;
  const [data, setData] = useState<ListResp | null>(null);
  const [mine, setMine] = useState<MineResp | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setData(await api.get<ListResp>("/api/v1/social/team/list"));
    } catch {
      setData({ enabled: false, list: [] });
    }
    try {
      setMine(await api.get<MineResp>("/api/v1/social/team/mine"));
    } catch {
      setMine({ enabled: false, list: [] });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function join(teamId: number) {
    if (busy !== null) return;
    setBusy(teamId);
    setMsg(null);
    try {
      await api.post("/api/v1/social/team/join", { team_id: teamId });
      setMsg(t.joinOk);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.joinFailed);
    } finally {
      setBusy(null);
    }
  }

  async function leave(teamId: number) {
    if (busy !== null) return;
    if (!window.confirm(t.leaveConfirm)) return;
    setBusy(teamId);
    setMsg(null);
    try {
      await api.post("/api/v1/social/team/leave", { team_id: teamId });
      setMsg(t.leaveOk);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.joinFailed);
    } finally {
      setBusy(null);
    }
  }

  const enabled = data?.enabled ?? false;
  const fmtTime = (s: string | null) => (s ? new Date(s).toLocaleString(dateLocale(locale)) : "-");

  if (data && !enabled) {
    return (
      <section className="baozi-panel preserve-list-card">
        <div className="preserve-list-card__heading">
          <h2>🤝 {t.title}</h2>
        </div>
        <p className="px-4 py-6 text-center text-xs text-sub">{t.disabled}</p>
      </section>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink" role="status">
          {msg}
        </p>
      )}

      <section className="baozi-panel preserve-list-card">
        <div className="preserve-list-card__heading">
          <h2>🤝 {t.title}</h2>
        </div>
        <p className="px-4 pb-2 text-xs text-sub">{t.note}</p>

        <div className="baozi-wide-table-scroll">
          <table className="nexus-table torrents-table">
            <thead>
              <tr>
                <th>{t.colTorrent}</th>
                <th className="w-28">{t.colLeader}</th>
                <th className="w-20 text-center">{t.colMembers}</th>
                <th className="w-20 text-center">{dict.endangered.colSeeders}</th>
                <th className="w-44">{t.colDeadline}</th>
                <th className="w-24">{t.colAction}</th>
              </tr>
            </thead>
            <tbody>
              {(data?.list ?? []).map((row) => (
                <tr key={row.team_id}>
                  <td>
                    <Link href={`/torrent/${row.torrent_id}`} className="text-link">
                      {row.torrent_name}
                    </Link>
                  </td>
                  <td>{row.leader_name}</td>
                  <td className="num text-center">
                    {row.member_count}/{row.team_size_max}
                  </td>
                  <td className="num text-center">{row.seeders}</td>
                  <td className="text-xs text-sub">{fmtTime(row.deadline_at)}</td>
                  <td>
                    {row.joined ? (
                      <span className="text-xs font-bold text-mint">{t.joined}</span>
                    ) : row.member_count >= row.team_size_max ? (
                      <span className="text-xs text-sub">{t.full}</span>
                    ) : (
                      <button
                        type="button"
                        disabled={busy !== null}
                        onClick={() => join(row.team_id)}
                        className="min-h-[32px] rounded-full border border-[var(--baozi-orange-dark)] px-3 text-xs font-bold text-[var(--baozi-orange-dark)] disabled:opacity-50"
                      >
                        {t.joinBtn}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {data !== null && (data.list ?? []).length === 0 && (
                <tr>
                  <td colSpan={6} className="py-6 text-center text-sub">
                    {t.empty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </section>

      <section className="baozi-panel preserve-list-card">
        <div className="preserve-list-card__heading">
          <h2>👥 {t.mineTitle}</h2>
        </div>

        {(mine?.list ?? []).map((team) => {
          const active = team.members.filter((m) => m.join_status === 1 || m.join_status === 0);
          const totalDelta = active.reduce((sum, m) => sum + Math.max(m.delta_seconds, 0), 0);
          return (
            <div key={team.team_id} className="mx-4 mb-3 rounded-[var(--r-md)] border border-line p-3">
              <div className="mb-2 flex flex-wrap items-center gap-2 text-sm">
                {team.torrent_id ? (
                  <Link href={`/torrent/${team.torrent_id}`} className="text-link font-bold">
                    {team.torrent_name}
                  </Link>
                ) : (
                  <span className="font-bold">{team.torrent_name ?? `#${team.team_id}`}</span>
                )}
                <span className="text-xs text-sub">
                  {t.memberCount.replace("{n}", String(active.length))}
                </span>
              </div>

              <table className="nexus-table text-xs">
                <tbody>
                  {active.map((m) => (
                    <tr key={m.uid}>
                      <td>
                        {m.username}
                        {m.is_leader && (
                          <span className="ml-2 rounded-full bg-sky-soft px-2 text-[11px] text-ink">
                            {t.leaderTag}
                          </span>
                        )}
                      </td>
                      <td className="num w-32 text-right text-sub">
                        {Math.floor(Math.max(m.delta_seconds, 0) / 3600)} h
                      </td>
                    </tr>
                  ))}
                  <tr>
                    <td className="colhead">{t.teamTotal}</td>
                    <td className="colhead num text-right">
                      {Math.floor(totalDelta / 3600)} h
                    </td>
                  </tr>
                </tbody>
              </table>

              {team.status === 1 && !team.viewer_is_leader && (
                <button
                  type="button"
                  disabled={busy !== null}
                  onClick={() => leave(team.team_id)}
                  className="mt-2 min-h-[30px] rounded-full border border-line px-3 text-xs text-sub disabled:opacity-50"
                >
                  {t.leaveBtn}
                </button>
              )}
            </div>
          );
        })}
        {mine !== null && (mine.list ?? []).length === 0 && (
          <p className="px-4 py-6 text-center text-xs text-sub">{t.mineEmpty}</p>
        )}
      </section>
    </div>
  );
}
