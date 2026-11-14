"use client";

import { useI18n } from "@/i18n/client";
import { useTenDraw } from "@/lib/ten-draw";
import { SwTenModal } from "@/components/game/sw-panels";

/**
 * 大转盘十连（波1 三兄弟拉齐）：scratch/jgg/capsule 都有十连，wheel 漏了。
 * 十次真实单抽（每次独立幂等键，lib/ten-draw 同一状态机），结果汇总浮层。
 * 自含触发钮 + 结果浮层——wheel/_inner 被行数基线钉死，只挂一行 <WheelTen/>。
 */
export function WheelTen({
  ticket,
  disabled,
}: {
  /** 票价（单次下注额，与主玩法一致） */
  ticket: number;
  disabled?: boolean;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tw = dict.games.wheel as unknown as Record<string, string>;
  const ten = useTenDraw<{ prize: string; payout: number; value?: number }>({
    path: "/api/v1/games/wheel",
    body: {},
    onError: () => {},
    netOf: (r) => (r.value ?? r.payout) - ticket,
    labelOf: (r) => r.prize,
  });

  const run = () => {
    // useTenDraw 的 body 在 hook 初始化时固定；wheel 无参数位，
    // 直接复用其 run（幂等键由 lib/ten-draw 逐发生成）
    void ten.run();
  };

  return (
    <>
      <button
        type="button"
        className="arc-chip num"
        disabled={disabled}
        onClick={run}
      >
        {tw.tenGo.replace("{n}", String(ticket * 10))}
      </button>
      <SwTenModal
        open={ten.open}
        title={tw.tenTitle}
        rows={ten.rows}
        totalLabel={(tw.tenTotal ?? "").replaceAll("{magic}", currency)}
        onClose={ten.close}
      />
    </>
  );
}
