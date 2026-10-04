"use client";

import { useEffect, useRef, useState } from "react";

/** 3×3 九宫格：外圈顺时针 8 格 = 8 个奖档，中心 = GO */
const RING = [0, 1, 2, 5, 8, 7, 6, 3];
const TOTAL_MS = 2400;

export interface JggPrize {
  label: string;
  /** magic | item；物品位的价值不在 payout 上 */
  kind?: "magic" | "item";
  /** 该档给玩家的魔力等值：魔力位=票价×倍数，物品位=anchor×件数 */
  value?: number;
  item_key?: string;
  qty?: number;
  /** 目录 arcade_items.icon：物品档显示自己的图标，不再一律 🎁 */
  icon?: string;
  weight_permille: number;
  payout: number;
}

/**
 * 九宫格抽奖灯阵。
 * 关键：跑马灯在**响应返回后**才启动（旧实现是请求发出就开始转，快网空转、慢网转完没结果）；
 * 奖池文案全部来自后端下发的 prizes（修「前端硬编码 100x 与后端不符」）。
 * 返本档（payout === 1）不占灯格：prizes 里它在列（公示/编辑器要它），
 * 但灯阵按位跳过——池子可含 9 档而灯板只有 8 格。
 */
export function JggGrid({
  prizes,
  ticket,
  resultIndex,
  replay,
  busy,
  disabled,
  reduced,
  onDraw,
  onLanded,
  goLabel,
  drawingLabel,
}: {
  prizes: JggPrize[];
  ticket: number;
  resultIndex: number | null;
  /** 服务端判定这局抽中的是返本档：灯阵不定位，直接走回摆+揭晓 */
  replay?: boolean;
  busy: boolean;
  disabled?: boolean;
  reduced: boolean;
  onDraw: () => void;
  onLanded?: () => void;
  goLabel: string;
  drawingLabel: string;
}) {
  const [lit, setLit] = useState<number | null>(null);
  const [landed, setLanded] = useState(false);
  const timer = useRef<number | null>(null);
  // 灯阵可见档 = 非返本档（payout===1 不占灯格）。`lampIndexOf[i]` =
  // 池内第 i 档在灯阵上的位置（返本档为 -1，中奖落在它上面时不点亮任何格）。
  const lamps = prizes
    .map((p, i) => ({ p, i }))
    .filter(({ p }) => p.payout !== 1);
  const lampIndexOf = prizes.map(
    (p) => lamps.findIndex((l) => l.p === p), // 引用相等：同一对象才映射
  );

  // 开抽瞬间清掉上一局的高亮/中奖态（否则新一局开始前上次的中奖格还亮着）
  useEffect(() => {
    if (busy) {
      setLanded(false);
      setLit(null);
    }
  }, [busy]);

  useEffect(() => {
    if (resultIndex === null) return;
    setLanded(false);
    // 返本档：不跑定位灯，转一圈后收在起点格附近直接揭晓（无中奖格可亮）
    const winLamp = replay ? -1 : (lampIndexOf[resultIndex] ?? -1);
    if (reduced) {
      setLit(winLamp >= 0 ? winLamp : null);
      setLanded(true);
      onLanded?.();
      return;
    }
    let step = 0;
    const n = Math.max(lamps.length, 1);
    const laps = n * 2;
    // near-miss 演出（趣味性批）：最后 2 格先冲过中奖格再回摆定格——
    // 相邻格擦肩的张力感；整体梯度 90→160→240→400→回摆 300
    const tick = () => {
      setLit(step % n);
      step++;
      if (step > laps + n) {
        // 回摆收尾：落在中奖格（返本局不亮任何格）
        setLit(winLamp >= 0 ? winLamp : null);
        setLanded(true);
        onLanded?.();
        return;
      }
      const remain = laps + n - step;
      const delay =
        remain <= 0
          ? 300
          : remain <= 2
            ? 420
            : remain <= 4
              ? 240
              : remain <= 7
                ? 160
                : 90;
      timer.current = window.setTimeout(tick, delay);
    };
    tick();
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resultIndex, reduced]);

  return (
    <div className="jgrid">
      <span className={`beam${busy && !reduced ? " run" : ""}`} aria-hidden />
      <div className="cells" aria-busy={busy}>
        {Array.from({ length: 9 }, (_, gi) => {
          if (gi === 4) {
            return (
              <button
                key="go"
                type="button"
                onClick={onDraw}
                disabled={busy || disabled}
                aria-label={`${goLabel} · ${ticket}`}
                className="go"
              >
                <span>{busy ? drawingLabel : goLabel}</span>
                <span className="tk">{ticket}</span>
              </button>
            );
          }
          const pi = RING.indexOf(gi);
          // 灯格与池档的对应：第 pi 个灯格 = lamps[pi]（返本档已被过滤，
          // 池内第 i 档 ≠ 灯阵第 i 格）。isLit/isWin 都按灯阵位比较。
          const lp = pi >= 0 ? lamps[pi] : undefined;
          const poolIdx = lp ? lp.i : -1;
          const p = lp?.p;
          if (!p) return <span key={gi} className="lamp" />;
          const isLit = lit === pi;
          const isWin = landed && resultIndex === poolIdx;
          // 物品位 payout 恒为 0，按 payout 判会把高价物品画成最低档样式
          const v = p.value ?? p.payout * ticket;
          const tier =
            v >= ticket * 10
              ? "jack"
              : v >= ticket * 5
                ? "high"
                : v >= ticket
                  ? "mid"
                  : "low";
          const cls = ["lamp", tier, isLit ? "lit" : "", isWin ? "win" : ""]
            .filter(Boolean)
            .join(" ");
          // 球面内容（样图④）：魔力档 = ✦ 星 + ×N 读数；物品档 = 自带图标
          // + 名字；未中奖档 = 无星 + 后端下发的「谢谢参与」类文案
          return (
            <div key={gi} className={cls}>
              {p.kind === "item" ? (
                <span className="bico" aria-hidden>
                  {p.icon ?? "🎁"}
                  {p.qty && p.qty > 1 && <i className="num">×{p.qty}</i>}
                </span>
              ) : (
                <span className="bstar" aria-hidden>
                  {p.payout === 0 ? "" : "✦"}
                </span>
              )}
              <span className="blabel num">
                {p.kind !== "item" && p.payout > 0 ? `×${p.payout}` : p.label}
              </span>
              <span className="ring" aria-hidden />
            </div>
          );
        })}
      </div>
    </div>
  );
}
