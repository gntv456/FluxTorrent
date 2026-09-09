import { BigSmall, JggCard, ScratchCard } from "@/components/game-actions";
import Link from "next/link";

export default function GamesPage() {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">娱乐屋</h1>
        <span className="text-sm text-sub">小火花，大快乐（理性娱乐）</span>
      </div>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <ScratchCard />
        <BigSmall />
        <JggCard />
        <Link
          href="/farm"
          className="flex min-h-[140px] flex-col items-center justify-center gap-2 rounded-[var(--r-lg)] border border-dashed border-sky bg-sky-soft/50 p-4 text-center transition-transform active:scale-[0.98]"
        >
          <span aria-hidden className="text-4xl">🌾</span>
          <span className="font-display text-lg">好学农场</span>
          <span className="text-xs text-sub">种下知识，收获火花</span>
        </Link>
      </div>
      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        规则：单次下注上限 1000 火花，每人每小时最多 60 局。娱乐有度，做种才是正道～
      </p>
    </div>
  );
}
