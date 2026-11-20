"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 卡池页的「登录态摘要」条（第二轮评审 P1）：
 *  首抽体验价提示 + SSR 合成进度此前是 RSC 静态 JSX——抽完卡后
 *  「首抽仅 1 券」还挂着、进度条停在旧百分比，像抽奖没生效。
 *  抽成 client 件监听 flux:gacha-drawn 自我刷新。 */
export function GachaPoolMeBar({
  initial,
  ssrSynth,
}: {
  initial: { hasCards: boolean; shardBalance: number } | null;
  ssrSynth: number;
}) {
  const { dict } = useI18n();
  const [me, setMe] = useState(initial);
  useEffect(() => {
    const onDrawn = () => {
      api
        .get<{ cards: unknown[]; shardBalance: number }>("/api/v1/gacha/me")
        .then((r) =>
          setMe({
            hasCards: (r.cards?.length ?? 0) > 0,
            shardBalance: r.shardBalance,
          })
        )
        .catch(() => {});
    };
    window.addEventListener("flux:gacha-drawn", onDrawn);
    return () => window.removeEventListener("flux:gacha-drawn", onDrawn);
  }, []);
  if (!me) return null;
  return (
    <>
      {!me.hasCards && (
        <p
          className="rounded-[var(--r-md)] bg-mint-soft px-3 py-2
            text-xs font-bold text-ink"
        >
          {dict.games.gachaFirstDraw}
        </p>
      )}
      {ssrSynth > 0 && (
        <div className="sw-synth">
          <div className="sw-synth-cap">{dict.games.gachaSynthCap}</div>
          <div className="sw-synth-row">
            <span className="sw-synth-track" aria-hidden>
              <i
                style={{
                  width:
                    `${Math.min(100, (me.shardBalance / ssrSynth) * 100)}%`,
                }}
              />
            </span>
            <b className="num">
              {dict.games.gachaSynthFrag
                .replace("{a}", String(me.shardBalance))
                .replace("{b}", String(ssrSynth))}
            </b>
          </div>
          <p className="sw-synth-note">
            {dict.games.gachaSynthNote.replace("{n}", String(ssrSynth))}
          </p>
        </div>
      )}
    </>
  );
}
