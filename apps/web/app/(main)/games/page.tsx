import { BigSmall, JggCard, ScratchCard } from "@/components/game-actions";
import { FunBox } from "@/components/fun-box";
import Link from "next/link";
import { getDict } from "@/i18n/server";

export default async function GamesPage() {
  const { dict, currency } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.games.title}</h1>
        <span className="text-sm text-sub">{dict.games.subtitle.replace("{magic}", currency)}</span>
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
          <span className="font-display text-lg">{dict.games.farmName}</span>
          <span className="text-xs text-sub">{dict.games.farmSub.replace("{magic}", currency)}</span>
        </Link>
      </div>
      <FunBox embedded />

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        {dict.games.rule.replace("{magic}", currency)}
      </p>
    </div>
  );
}
