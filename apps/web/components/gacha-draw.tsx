"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { playSfx } from "@/lib/sfx";
import { WinBurst } from "@/components/game/win-burst";
import type { GachaRarityRow } from "@/app/(main)/gacha/_inner";

/**
 * 抽卡主玩法（波1 P0）：后端 POST /gacha/draw 全链在位（保底软硬/碎片
 * 聚合/幂等重放），此前 Web 端零调用——/gacha 只是只读公示页。
 *
 * 口径：
 * - 幂等键 ui-gacha-<ts>-<rand>，重放由后端同 key 返回同批结果
 * - 结果揭晓走三档庆祝（口径同娱乐屋）：金档 WinBurst + 上行音、
 *   普通卡 pop、十连逐张翻转错峰（arcade-fx.css gcFlip）
 * - 券不足/首抽体验价文案直接展示后端错误信封（含获取路径提示）
 * - 保底进度 pityAt 由后端逐抽下发（pityAt = 距硬保底计数）
 */

export interface DrawRow {
  seq: number;
  type: string; // card | shard
  r: string | null;
  shards: number;
  pityAt: number;
  wasPity: boolean;
  isNew?: boolean;
  cardId?: number;
}

interface DrawResp {
  replayed: boolean;
  bannerId: number;
  rows: DrawRow[];
  ticketBalance: number;
  shardBalance: number;
}

export function GachaDraw({
  bannerId,
  ticketCost,
  ticketBalance,
  rarities,
}: {
  bannerId: number;
  /** 票面价（首抽体验价由后端定，前端只展示扣后余额） */
  ticketCost: number;
  ticketBalance: number;
  rarities: GachaRarityRow[];
}) {
  const { dict } = useI18n();
  const tg = dict.games as unknown as Record<string, string>;
  const [bal, setBal] = useState(ticketBalance);
  const [busy, setBusy] = useState(false);
  const [rows, setRows] = useState<DrawRow[] | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => setBal(ticketBalance), [ticketBalance]);

  const rarityOf = useCallback(
    (r: string | null) => (r ? rarities.find((x) => x.key === r) : undefined),
    [rarities],
  );

  const draw = useCallback(
    async (count: 1 | 10) => {
      if (busy) return;
      setBusy(true);
      setErr("");
      try {
        const resp = await api.post<DrawResp>("/api/v1/gacha/draw", {
          banner_id: bannerId,
          count,
          idempotency_key: `ui-gacha-${Date.now()}-${
            Math.floor(Math.random() * 1e6)
          }`,
        });
        setRows(resp.rows);
        setBal(resp.ticketBalance);
        const gold = resp.rows.some((r) =>
          (r.r ?? "").length > 0 && (rarityOf(r.r)?.stars ?? 0) >= 5,
        );
        playSfx(gold ? "jackpot" : "win");
      } catch (e) {
        setErr(e instanceof ApiError ? e.message : String(e));
      } finally {
        setBusy(false);
      }
    },
    [bannerId, busy, rarityOf],
  );

  const starsOf = (r: string | null) => rarityOf(r)?.stars ?? 1;
  const faceStyle = (r: string | null): React.CSSProperties => {
    const m = rarityOf(r);
    if (!m?.bg1) return {};
    return {
      background: `linear-gradient(160deg, ${m.bg1}, ${m.bg2 ?? m.bg1})`,
      color: m.ink ?? undefined,
      borderColor: m.frame ?? undefined,
    };
  };

  return (
    <div className="sw-gacha">
      <div className="sw-stat-row">
        <div className="sw-stat">
          <div className="sw-stat-lb">{tg.gachaTickets}</div>
          <div className="sw-stat-vl num">{bal}</div>
        </div>
        <div className="sw-stat">
          <div className="sw-stat-lb">{tg.gachaPityCap}</div>
          <div className="sw-stat-vl num gold">
            {(rows?.[rows.length - 1]?.pityAt ?? 0) || "—"}
          </div>
        </div>
      </div>
      {err && <p className="sw-gacha-err text-sm">{err}</p>}
      <div className="sw-gacha-acts">
        <button
          type="button"
          className="sw-album-btn gold"
          disabled={busy}
          onClick={() => void draw(1)}
        >
          {busy ? tg.gachaDrawing : tg.gachaDrawOne.replace("{n}", String(bal))}
        </button>
        <button
          type="button"
          className="sw-album-btn"
          disabled={busy || bal < ticketCost * 10}
          onClick={() => void draw(10)}
        >
          {tg.gachaDrawTen.replace("{n}", String(ticketCost * 10))}
        </button>
      </div>
      {rows && (
        <div
          className="sw-gacha-results"
          key={rows.map((r) => r.seq).join(",")}
        >
          {(rows[0]?.wasPity || rows.some((r) => starsOf(r.r) >= 5)) && (
            <WinBurst fire />
          )}
          {rows.map((r, i) => (
            <div
              key={r.seq}
              className="gc-flip"
              style={{ animationDelay: `${i * 70}ms` }}
            >
              <div className="sw-album-face" style={faceStyle(r.r)}>
                <span className="sw-album-face-r num">
                  {r.type === "shard"
                    ? `+${r.shards}`
                    : r.type === "miss"
                      ? "－"
                      : "★".repeat(starsOf(r.r))}
                </span>
                <span className="sw-album-face-name">
                  {r.type === "shard"
                    ? tg.gachaShardDrop
                    : r.type === "miss"
                      ? tg.gachaMiss
                      : r.isNew
                        ? tg.gachaNew
                        : r.r ?? ""}
                </span>
              </div>
              {r.wasPity && (
                <span className="sw-gacha-pity num">{tg.gachaPityTag}</span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
