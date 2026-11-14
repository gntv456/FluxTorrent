"use client";

/**
 * G31 收藏面（方案 §2 图鉴判据的落地）：「已获得」与持有数解耦——lit_at
 * 是首次点亮时间，分解重复卡后灯不灭（ever-lit）。三动作按钮直连
 * craft 端点，错误信封文案直接展示（400 原因 / 409 超次）。
 * 稀有度色 token 全部来自后端 rarities 行表，前端不持色表（样张判据）。
 *
 * 2026-10 样图⑧对齐：统计行/卡面贴纸/空态与「去抽卡」CTA 挂甜梦皮肤
 * （sw-album-*），文案收编 i18n（原先硬编码中文）。
 */
import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { GachaRecent } from "@/components/gacha-recent";
import type { GachaRarityRow } from "@/app/(main)/gacha/_inner";

export interface MyCard {
  id: number;
  key: string;
  name: string;
  rarity: string;
  held: number;
  lv: number;
  lvMax: number;
  dupeShards: number;
  synthShards: number;
  litAt: string;
  exchanged: boolean;
}

export interface MePayload {
  ticketBalance: number;
  shardBalance: number;
  cards: MyCard[];
  litCount: number;
  litTotal: number;
  /** 近 30 抽（波1 起前端开始渲染；此前后端一直下发、前端丢弃） */
  recent?: Array<{
    seq: number;
    output_type: string;
    rarity: string | null;
    shards: number;
    was_pity: boolean;
  }>;
}

export function GachaAlbum({
  rarities,
  initial,
}: {
  rarities: GachaRarityRow[];
  initial: MePayload | null;
}) {
  const { dict } = useI18n();
  const tg = dict.games as unknown as Record<string, string>;
  const [me, setMe] = useState<MePayload | null>(initial);
  const [busy, setBusy] = useState<number | null>(null);
  const [err, setErr] = useState("");

  const reload = useCallback(async () => {
    try {
      setMe(await api.get<MePayload>("/api/v1/gacha/me"));
    } catch {
      /* 保持旧数据；错误在动作路径上展示 */
    }
  }, []);

  useEffect(() => {
    if (!initial) void reload();
  }, [initial, reload]);

  const act = useCallback(
    async (path: string, cardId: number) => {
      setBusy(cardId);
      setErr("");
      try {
        await api.post(`/api/v1/gacha/${path}`, {
          card_id: cardId,
          idempotency_key: `ui-${path}-${cardId}-${Date.now()}`,
        });
        await reload();
      } catch (e) {
        setErr(e instanceof ApiError ? e.message : String(e));
      } finally {
        setBusy(null);
      }
    },
    [reload],
  );

  if (!me) {
    return (
      <div className="sw-album-panel p-6 text-sm">{tg.gachaLoading ?? "…"}</div>
    );
  }
  const rarityOf = (r: string) => rarities.find((x) => x.key === r);
  /** 卡面贴纸底色：后端 rarities token（暗底渐变 + 亮墨），无 token 时退中性面 */
  const faceStyle = (r: string): React.CSSProperties => {
    const m = rarityOf(r);
    if (!m?.bg1) return {};
    return {
      background: `linear-gradient(160deg, ${m.bg1}, ${m.bg2 ?? m.bg1})`,
      color: m.ink ?? undefined,
      borderColor: m.frame ?? undefined,
    };
  };
  return (
    <div className="sw-album">
      {err && (
        <div className="sw-album-panel p-3 text-sm text-destructive">{err}</div>
      )}
      <section className="sw-album-panel">
        <header className="sw-album-head">
          <h2>{tg.gachaMyAlbum}</h2>
        </header>
        <div className="sw-stat-row">
          <div className="sw-stat">
            <div className="sw-stat-lb">{tg.gachaTickets}</div>
            <div className="sw-stat-vl num">{me.ticketBalance}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{tg.gachaShards}</div>
            <div className="sw-stat-vl num gold">{me.shardBalance}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{tg.gachaLitLabel}</div>
            <div className="sw-stat-vl num green">
              {me.litCount}/{me.litTotal}
            </div>
          </div>
        </div>
        <div className="sw-album-grid">
          {me.cards.map((c) => {
            const m = rarityOf(c.rarity);
            return (
              <div key={c.id} className="sw-album-card">
                <div className="sw-album-face" style={faceStyle(c.rarity)}>
                  <span className="sw-album-face-name">{c.name}</span>
                  <span className="sw-album-face-r num">
                    {"★".repeat(Math.max(1, m?.stars ?? 1))}
                  </span>
                </div>
                <div className="sw-album-meta">
                  {tg.gachaHeld.replace("{n}", String(c.held))} · Lv.{c.lv}/
                  {c.lvMax}
                  {c.exchanged && (
                    <span className="sw-album-exch">{tg.gachaExchanged}</span>
                  )}
                </div>
                <div className="sw-album-acts">
                  <button
                    className="sw-album-btn"
                    disabled={busy === c.id || c.held <= 1 || c.dupeShards <= 0}
                    onClick={() => void act("dismantle", c.id)}
                  >
                    {tg.gachaDismantle.replace("{n}", String(c.dupeShards))}
                  </button>
                  <button
                    className="sw-album-btn gold"
                    disabled={
                      busy === c.id || c.exchanged || c.synthShards <= 0
                    }
                    title={c.exchanged ? tg.gachaExchanged : undefined}
                    onClick={() => void act("exchange", c.id)}
                  >
                    {tg.gachaExchange.replace("{n}", String(c.synthShards))}
                  </button>
                  <button
                    className="sw-album-btn"
                    disabled={busy === c.id || c.lv >= c.lvMax}
                    onClick={() => void act("levelup", c.id)}
                  >
                    {c.lv >= c.lvMax ? tg.gachaMaxed : tg.gachaLevelup}
                  </button>
                </div>
              </div>
            );
          })}
          {me.cards.length === 0 && (
            <div className="sw-album-empty">
              <span className="sw-album-empty-face" aria-hidden>
                <ArcadeGlyph k="gacha" />
              </span>
              <p>{tg.gachaAlbumEmpty}</p>
            </div>
          )}
        </div>
      </section>
      {/* CTA 直指本页卡池 tab 的抽卡区（/gacha#draw）——原先错链 /games/jgg
          是九宫格，抽卡玩法根本不在那（波1 P0 修复） */}
      <a href="/gacha#draw" className="sw-album-cta">
        {tg.gachaDrawCta}
      </a>
      {me.recent && me.recent.length > 0 && (
        <section className="sw-album-panel">
          <header className="sw-album-head">
            <h2>{tg.gachaRecentCap}</h2>
          </header>
          <GachaRecent
            rows={me.recent.map((r) => ({
              seq: r.seq,
              type: r.output_type,
              rarity: r.rarity,
              shards: r.shards,
              wasPity: r.was_pity,
            }))}
            starOf={(r) =>
              rarities.find((x) => x.key === r)?.stars ?? 1
            }
          />
        </section>
      )}
    </div>
  );
}
