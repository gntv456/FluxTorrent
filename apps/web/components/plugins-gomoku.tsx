"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** hxpt 插件移植前台（从 components/plugins.tsx 按域拆出）：
 *  五子棋 GomokuBoard——15x15 棋盘、建房/加入、4s 轮询刷新。 */

interface GomokuGame {
  id: number;
  black_id: number;
  white_id: number | null;
  board: string;
  turn: string;
  winner_id: number | null;
}

// 加入对局 ID 的输入框 / 加入按钮
const JOIN_INPUT =
  "min-h-[40px] w-32 rounded-[var(--r-md)] border border-line " +
  "px-3 text-sm";
const JOIN_BTN =
  "min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white";

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

  const load = useCallback(
    async (gid: number) => {
      try {
        const g = await api.get<GomokuGame>(`/api/v1/gomoku/games/${gid}`);
        setGame(g);
      } catch (e) {
        setMsg(e instanceof ApiError ? e.message : dict.common.loadFailed);
      }
    },
    [dict],
  );
  useEffect(() => {
    if (gameId === null) return;
    load(gameId);
    const timer = setInterval(() => load(gameId), 4000);
    return () => clearInterval(timer);
  }, [gameId, load]);

  async function act(fn: () => Promise<void>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) setMsg(ok);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
      setTimeout(() => setMsg(null), 3000);
    }
  }

  const cells = game ? game.board.padEnd(225, ".") : "";
  const myColor = game
    ? game.black_id === meId
      ? "b"
      : game.white_id === meId
        ? "w"
        : null
    : null;
  const myTurn =
    game !== null &&
    myColor !== null &&
    game.turn === myColor &&
    game.winner_id === null;

  if (gameId === null) {
    return (
      <div className="baozi-panel flex flex-col items-start gap-3 p-4">
        <p className="text-sm text-sub">{t.lobbyHint}</p>
        <div className="flex gap-2">
          <button
            className="baozi-button"
            disabled={busy}
            onClick={() =>
              act(async () => {
                const r = await api.post<{ id: number }>(
                  "/api/v1/gomoku/games",
                );
                setGameId(r.id);
                window.history.replaceState(null, "", `/gomoku?id=${r.id}`);
              })
            }
          >
            {t.btnCreate}
          </button>
          <input
            className={JOIN_INPUT}
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
          {game && (
            <span className="ml-2 text-xs font-normal text-sub">
              {game.winner_id !== null
                ? game.winner_id === meId
                  ? t.youWin
                  : t.youLose
                : myColor === null
                  ? t.spectator
                  : myTurn
                    ? t.yourMove
                    : t.waitOpponent}
            </span>
          )}
        </p>
        {game && myColor === null && game.white_id === null && (
          <button
            className={JOIN_BTN}
            disabled={busy}
            onClick={() =>
              act(async () => {
                await api.post(`/api/v1/gomoku/games/${gameId}/join`);
                await load(gameId);
              }, t.joined)
            }
          >
            {t.btnJoin}
          </button>
        )}
      </div>
      <div className="gomoku-board mx-auto" role="grid" aria-label="gomoku">
        {Array.from({ length: 225 }, (_, i) => {
          const c = cells[i];
          const clickable = myTurn && c === ".";
          return (
            <button
              key={i}
              className={
                "gomoku-cell " +
                `${c === "b" ? "gomoku-b" : c === "w" ? "gomoku-w" : ""}`
              }
              disabled={!clickable || busy}
              onClick={() =>
                act(async () => {
                  await api.post(`/api/v1/gomoku/games/${gameId}/move`, {
                    pos: i,
                  });
                  await load(gameId);
                })
              }
              aria-label={`(${Math.floor(i / 15)},${i % 15})`}
            />
          );
        })}
      </div>
      {msg && <p className="text-center text-xs text-sub">{msg}</p>}
    </div>
  );
}
