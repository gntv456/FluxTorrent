import { ScratchCard, BigSmall } from "@/components/game-actions";

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
      </div>
      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        规则：单次下注上限 1000 火花，每人每小时最多 60 局。娱乐有度，做种才是正道～
      </p>
    </div>
  );
}
