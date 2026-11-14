"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { SwRules } from "@/components/game/sw-enrich";

/**
 * 玩法规则条（波1 三兄弟拉齐）：fishing 已有 SwRules，scratch/jgg/capsule/
 * wheel 缺——统一补齐。文案 = 该玩法 dict 的 poolNote（奖池口径一句），
 * 右链固定指 /games/odds（概率公示，波1 起覆盖 7/9 玩法）。
 * 侧栏一行挂载：<GameRules game="wheel" />，不动被行数基线钉死的 _inner。
 */
export function GameRules({ game }: { game: string }) {
  const { dict } = useI18n();
  const seg = (dict.games as unknown as Record<string, Record<string, string>>)[
    game
  ];
  const text = seg?.poolNote ?? "";
  if (!text) return null;
  return (
    <SwRules
      left={text}
      linkLabel={dict.games.oddsLink}
      linkHref="/games/odds"
    />
  );
}
