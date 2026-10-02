"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";

/** 「战绩与图鉴」子页入口（样图 2026-10 补页批）：舞台下的小字链接。
 *  客户端件（游戏页全是 "use client"）；文案在 dict.games.sub.recordsTitle。 */
export function SubpageLink({ game }: { game: string }) {
  const { dict } = useI18n();
  return (
    <Link
      href={`/games/${game}/records`}
      className="text-xs font-bold text-[var(--sky-deep)]"
    >
      {dict.games.sub.recordsTitle} →
    </Link>
  );
}
