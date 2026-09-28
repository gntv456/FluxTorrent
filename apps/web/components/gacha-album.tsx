"use client";

/**
 * G31 收藏面（方案 §2 图鉴判据的落地）：「已获得」与持有数解耦——lit_at
 * 是首次点亮时间，分解重复卡后灯不灭（ever-lit）。三动作按钮直连
 * craft 端点，错误信封文案直接展示（400 原因 / 409 超次）。
 * 稀有度色 token 全部来自后端 rarities 行表，前端不持色表（样张判据）。
 */
import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
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
}

export function GachaAlbum({
  rarities,
  initial,
}: {
  rarities: GachaRarityRow[];
  initial: MePayload | null;
}) {
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
    return <div className="card p-6 text-sm text-muted">加载中…</div>;
  }
  const color = (r: string) =>
    rarities.find((x) => x.key === r)?.frame ?? undefined;
  return (
    <div className="flex flex-col gap-4">
      <div className="card flex flex-wrap gap-x-6 gap-y-1 p-4 text-sm">
        <span>抽卡券 <b className="tabular-nums">{me.ticketBalance}</b></span>
        <span>碎片 <b className="tabular-nums">{me.shardBalance}</b></span>
        <span className="ml-auto text-muted">
          图鉴 {me.litCount}/{me.litTotal}
        </span>
      </div>
      {err && (
        <div className="card p-3 text-sm text-destructive">
          {err}
        </div>
      )}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {me.cards.map((c) => (
          <div key={c.id} className="card flex flex-col gap-2 p-4">
            <div className="flex items-baseline justify-between gap-2">
              <span
                className="font-semibold"
                style={{ color: color(c.rarity) }}
              >
                {c.name}
              </span>
              <span className="text-xs text-muted">{c.rarity}</span>
            </div>
            <div className="text-xs text-muted">
              持有 ×{c.held} ｜ Lv.{c.lv}/{c.lvMax} ｜ 点亮 {c.litAt}
            </div>
            <div className="mt-auto flex flex-wrap gap-2">
              <button
                className="btn text-xs"
                disabled={busy === c.id || c.held <= 1 || c.dupeShards <= 0}
                onClick={() => void act("dismantle", c.id)}
              >
                分解 +{c.dupeShards}
              </button>
              <button
                className="btn text-xs"
                disabled={busy === c.id || c.exchanged || c.synthShards <= 0}
                title={c.exchanged ? "本季已兑换过" : undefined}
                onClick={() => void act("exchange", c.id)}
              >
                兑换 {c.synthShards}
              </button>
              <button
                className="btn text-xs"
                disabled={busy === c.id || c.lv >= c.lvMax}
                onClick={() => void act("levelup", c.id)}
              >
                {c.lv >= c.lvMax ? "已满级" : "升级"}
              </button>
            </div>
          </div>
        ))}
        {me.cards.length === 0 && (
          <div className="card p-6 text-sm text-muted">
            还没有卡——先去抽一发吧。
          </div>
        )}
      </div>
    </div>
  );
}
