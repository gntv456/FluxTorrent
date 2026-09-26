"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

import { FrameShop } from "./plugins-frames";
import { GomokuBoard } from "./plugins-gomoku";

/** hxpt 插件移植前台：大赛 contest / 头像挂件 avatar_frame / 五子棋 wuziqi
 *  （勋章墙已按域拆出 @/components/medal-wall）
 *  头像挂件拆至 ./plugins-frames.tsx；五子棋拆至 ./plugins-gomoku.tsx。 */

interface Contest {
  id: number;
  title: string;
  descr: string | null;
  starts_at: string;
  ends_at: string;
  is_active: boolean;
  entries: number;
  leader: string | null;
  leader_score: number | null;
}

// ============ 大赛 ============

export function ContestBoard() {
  const { dict } = useI18n();
  const t = dict.contests2;
  const [rows, setRows] = useState<Contest[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .get<Contest[]>("/api/v1/contests")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);
  useEffect(load, [load]);

  async function join(id: number) {
    setBusy(true);
    try {
      await api.post(`/api/v1/contests/${id}/join`);
      setMsg(t.joined);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
      setTimeout(() => setMsg(null), 2500);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <div className="baozi-wide-table-scroll">
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{t.colTitle}</td>
            <td className="colhead">{t.colPeriod}</td>
            <td className="colhead">{t.colEntries}</td>
            <td className="colhead">{t.colLeader}</td>
            <td className="colhead text-right">{dict.cmgmt.colActions}</td>
          </tr>
          {rows.map((c) => (
            <tr key={c.id}>
              <td>
                <span className="font-bold text-ink">{c.title}</span>
                {c.is_active && <span className="contest-live">{t.live}</span>}
                {c.descr && <p className="mt-1 text-xs text-sub">{c.descr}</p>}
              </td>
              <td className="text-xs text-sub">
                {new Date(c.starts_at).toLocaleDateString("zh-CN")} ~{" "}
                {new Date(c.ends_at).toLocaleDateString("zh-CN")}
              </td>
              <td className="num">{c.entries}</td>
              <td>{c.leader ? `${c.leader} (${c.leader_score ?? 0})` : "—"}</td>
              <td className="text-right">
                {c.is_active && (
                  <button
                    className="cmgmt-act"
                    disabled={busy}
                    onClick={() => join(c.id)}
                  >
                    {t.btnJoin}
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
    </div>
  );
}

export { FrameShop } from "./plugins-frames";
export type { Frame } from "./plugins-frames";
export { GomokuBoard } from "./plugins-gomoku";
