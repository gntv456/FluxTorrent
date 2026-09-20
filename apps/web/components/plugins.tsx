"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** hxpt 插件移植前台：大赛 contest / 头像挂件 avatar_frame / 五子棋 wuziqi
 *  （勋章墙已按域拆出 @/components/medal-wall） */

interface Contest {
  id: number; title: string; descr: string | null;
  starts_at: string; ends_at: string; is_active: boolean;
  entries: number; leader: string | null; leader_score: number | null;
}
interface Frame { id: number; name: string; css: string; image_url?: string | null; price: number }

// ============ 大赛 ============

export function ContestBoard() {
  const { dict } = useI18n();
  const t = dict.contests2;
  const [rows, setRows] = useState<Contest[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api.get<Contest[]>("/api/v1/contests").then(setRows).catch(() => setRows([]));
  }, []);
  useEffect(load, [load]);

  async function join(id: number) {
    setBusy(true);
    try {
      await api.post(`/api/v1/contests/${id}/join`);
      setMsg(t.joined); load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally { setBusy(false); setTimeout(() => setMsg(null), 2500); }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
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
                  <button className="cmgmt-act" disabled={busy} onClick={() => join(c.id)}>{t.btnJoin}</button>
                )}
              </td>
            </tr>
          ))}
          {rows.length === 0 && <tr><td colSpan={5} className="py-6 text-center text-sub">{t.empty}</td></tr>}
        </tbody>
      </table>
    </div>
  );
}

// ============ 头像挂件 ============

export function FrameShop({ current, onChange }: { current: number | null; onChange?: (id: number | null) => void }) {
  const { dict } = useI18n();
  const t = dict.frames;
  const [frames, setFrames] = useState<Frame[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.get<Frame[]>("/api/v1/avatar-frames").then(setFrames).catch(() => setFrames([]));
  }, []);

  async function equip(id: number | null) {
    setBusy(true);
    try {
      await api.put("/api/v1/me/avatar-frame", { frame_id: id });
      setMsg(id === null ? t.removed : t.equipped);
      onChange?.(id);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally { setBusy(false); setTimeout(() => setMsg(null), 2500); }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
      <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3">
        <div className="baozi-panel flex flex-col items-center gap-2 p-4">
          <span className={`avatar-frame-demo ${current === null ? "frame-on" : ""}`} aria-hidden>🙂</span>
          <p className="text-xs font-bold text-ink">{t.none}</p>
          <button className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold disabled:opacity-50"
            disabled={busy || current === null} onClick={() => equip(null)}>{t.btnRemove}</button>
        </div>
        {frames.map((f) => (
          <div key={f.id} className="baozi-panel flex flex-col items-center gap-2 p-4">
            <span
              className={`avatar-frame-demo frame-style-${f.id} relative ${current === f.id ? "frame-on" : ""}`}
              style={f.image_url ? undefined : frameStyle(f.css)}
              aria-hidden
            >
              🙂
              {f.image_url ? (
                // eslint-disable-next-line @next/next/no-img-element
                <img src={f.image_url} alt="" className="pointer-events-none absolute inset-0 z-10 h-full w-full select-none object-fill" />
              ) : null}
            </span>
            <p className="text-xs font-bold text-ink">{f.name}</p>
            <p className="text-xs text-sub">{f.price > 0 ? `✨ ${f.price}` : t.free}</p>
            <button className="min-h-[32px] rounded-full bg-sky px-3 text-xs font-bold text-white disabled:opacity-50"
              disabled={busy || current === f.id} onClick={() => equip(f.id)}>
              {current === f.id ? t.on : t.btnEquip}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

function frameStyle(css: string): React.CSSProperties {
  // css 形如 "border-color:#e14d4d" —— 仅放行 border-color/box-shadow 两个属性
  const style: Record<string, string> = {};
  for (const decl of css.split(";")) {
    const [k, v] = decl.split(":").map((s) => s?.trim());
    if (k && v && ["border-color", "box-shadow"].includes(k)) style[k] = v;
  }
  return style as React.CSSProperties;
}

// ============ 五子棋 ============

interface GomokuGame {
  id: number; black_id: number; white_id: number | null;
  board: string; turn: string; winner_id: number | null;
}

export function GomokuBoard({ meId }: { meId: number }) {
  const { dict } = useI18n();
  const t = dict.gomoku;
  const [game, setGame] = useState<GomokuGame | null>(null);
  const [gameId, setGameId] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // 加入已有对局（URL ?id=）
  useEffect(() => {
    const q = new URLSearchParams(window.location.search).get("id");
    if (q) setGameId(Number(q));
  }, []);

  const load = useCallback(async (gid: number) => {
    try {
      const g = await api.get<GomokuGame>(`/api/v1/gomoku/games/${gid}`);
      setGame(g);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [dict]);
  useEffect(() => {
    if (gameId === null) return;
    load(gameId);
    const timer = setInterval(() => load(gameId), 4000);
    return () => clearInterval(timer);
  }, [gameId, load]);

  async function act(fn: () => Promise<void>, ok?: string) {
    setBusy(true);
    try { await fn(); if (ok) setMsg(ok); }
    catch (e) { setMsg(e instanceof ApiError ? e.message : dict.common.networkError); }
    finally { setBusy(false); setTimeout(() => setMsg(null), 3000); }
  }

  const cells = game ? game.board.padEnd(225, ".") : "";
  const myColor = game ? (game.black_id === meId ? "b" : game.white_id === meId ? "w" : null) : null;
  const myTurn = game !== null && myColor !== null && game.turn === myColor && game.winner_id === null;

  if (gameId === null) {
    return (
      <div className="baozi-panel flex flex-col items-start gap-3 p-4">
        <p className="text-sm text-sub">{t.lobbyHint}</p>
        <div className="flex gap-2">
          <button className="baozi-button" disabled={busy}
            onClick={() => act(async () => {
              const r = await api.post<{ id: number }>("/api/v1/gomoku/games");
              setGameId(r.id);
              window.history.replaceState(null, "", `/gomoku?id=${r.id}`);
            })}>{t.btnCreate}</button>
          <input
            className="min-h-[40px] w-32 rounded-[var(--r-md)] border border-line px-3 text-sm"
            placeholder={t.joinId}
            onKeyDown={(e) => {
              const v = (e.target as HTMLInputElement).value.trim();
              if (e.key === "Enter" && v) setGameId(Number(v));
            }}
          />
        </div>
        {msg && <p className="text-xs text-sub">{msg}</p>}
      </div>
    );
  }

  return (
    <div className="baozi-panel flex flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-sm font-bold text-ink">
          {t.game} #{gameId}
          {game && <span className="ml-2 text-xs font-normal text-sub">
            {game.winner_id !== null
              ? (game.winner_id === meId ? t.youWin : t.youLose)
              : myColor === null ? t.spectator
              : myTurn ? t.yourMove : t.waitOpponent}
          </span>}
        </p>
        {game && myColor === null && game.white_id === null && (
          <button className="min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white" disabled={busy}
            onClick={() => act(async () => { await api.post(`/api/v1/gomoku/games/${gameId}/join`); await load(gameId); }, t.joined)}>
            {t.btnJoin}
          </button>
        )}
      </div>
      <div className="gomoku-board mx-auto" role="grid" aria-label="gomoku">
        {Array.from({ length: 225 }, (_, i) => {
          const c = cells[i];
          const clickable = myTurn && c === ".";
          return (
            <button key={i} className={`gomoku-cell ${c === "b" ? "gomoku-b" : c === "w" ? "gomoku-w" : ""}`}
              disabled={!clickable || busy}
              onClick={() => act(async () => { await api.post(`/api/v1/gomoku/games/${gameId}/move`, { pos: i }); await load(gameId); })}
              aria-label={`(${Math.floor(i / 15)},${i % 15})`} />
          );
        })}
      </div>
      {msg && <p className="text-center text-xs text-sub">{msg}</p>}
    </div>
  );
}
