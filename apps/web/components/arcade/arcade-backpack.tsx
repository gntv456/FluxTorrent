"use client";

/**
 * 我的背包：奖池物品位发出来的东西，按「发放账 − 消耗账」聚合回玩家眼前。
 *
 * 件数不在前端累加，也不另存一份「已用数」—— 一律由后端从两张账反推
 * （第二份清单必然漂移）。这里只做呈现，以及把「使用」这一笔交出去。
 *
 * 使用失败必须当场说话：奖品是站内虚拟物品，「点了没反应」和「这件东西本来
 * 就不能用」是两回事，静默 refresh 会把配置问题伪装成网络问题。
 */

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useI18n } from "@/i18n/client";
import { api } from "@/lib/api-client";

export interface ArcadePackItem {
  key: string;
  name: string;
  icon: string;
  kind: string;
  anchor: number;
  anchor_src: string;
  enabled: boolean;
  qty: number;
  granted?: number;
  used?: number;
  use_kind?: string;
  use_ref?: string;
  last_game: string | null;
  last_at: string | null;
}

export interface ArcadePackData {
  total: number;
  kinds: number;
  items: ArcadePackItem[];
}

const fmt = (n: number) => Math.round(n).toLocaleString("en-US");

export function ArcadeBackpack({ pack }: { pack: ArcadePackData }) {
  const { dict, currency } = useI18n();
  const t = dict.games.arcade;
  const router = useRouter();
  const titles = dict.games as unknown as Record<string, { title?: string }>;
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<{ key: string; text: string } | null>(null);
  const gameName = (g: string | null) => (g ? (titles[g]?.title ?? g) : "");

  // 名字不能以 use 开头：ESLint 的 rules-of-hooks 会把它当成 Hook 报错
  async function redeemOne(x: ArcadePackItem) {
    setBusy(x.key);
    setNote(null);
    try {
      const r = await api.post<{
        use_kind: string;
        spark?: number;
        granted_as?: string;
        held: number;
      }>(
        "/api/v1/games/arcade/backpack/use",
        { item_key: x.key, qty: 1 },
      );
      setNote({
        key: x.key,
        text:
          r.use_kind === "sku"
            ? t.packGotSku.replace("{s}", r.granted_as ?? "")
            : t.packGotSpark
                .replace("{n}", fmt(r.spark ?? 0))
                .replace("{c}", currency),
      });
      router.refresh();
    } catch (e) {
      setNote({ key: x.key, text: (e as Error).message });
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="arc-sec">
      <h3>
        {t.packTitle}
        {pack.kinds > 0 && (
          <span className="arc-note">
            {" "}
            · {pack.kinds} / {pack.total}
          </span>
        )}
      </h3>
      <div className="arc-shelf">
        {pack.items.map((x) => {
          const uk = x.use_kind ?? "collect";
          const usable = uk !== "collect" && x.enabled && x.qty > 0;
          return (
            <div key={x.key} className="arc-goods">
              <div className="gc" aria-hidden>
                {x.icon}
              </div>
              <div className="gn">
                {x.name}
                {x.qty > 1 && <span className="qty">×{x.qty}</span>}
              </div>
              <div className="gv">
                {gameName(x.last_game)}
                {x.anchor > 0 && ` · ${fmt(x.anchor)} ${currency}`}
              </div>
              {x.used ? (
                <div className="arc-note">
                  {t.packUsed} {x.used} / {x.granted ?? x.qty}
                </div>
              ) : null}
              {!x.enabled && <span className="zero">{t.packOff}</span>}
              {uk === "collect" && (
                <span className="arc-chip mini">{t.packCollect}</span>
              )}
              {usable && (
                <button
                  className="arc-btn tiny"
                  disabled={busy === x.key}
                  onClick={() => void redeemOne(x)}
                >
                  {busy === x.key ? t.packUsing : t.packUse}
                </button>
              )}
              {note?.key === x.key && (
                <div className="arc-note">{note.text}</div>
              )}
            </div>
          );
        })}
        {pack.items.length === 0 && (
          <div className="arc-note">{t.packEmpty}</div>
        )}
      </div>
    </section>
  );
}
