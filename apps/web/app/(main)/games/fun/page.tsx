import Link from "next/link";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";
import { FunBox } from "@/components/fun-box";

/**
 * 趣味盒独立页：段子投票（旧站 fun.php 口径）。
 * 它是社交内容、不是下注玩法 —— 从娱乐屋大厅游戏网格里移出，单独一页承载。
 */
export default async function FunPage() {
  const gate = await requireModule("games");
  if (gate) return gate;
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">FunBox</div>
          <h1 className="font-display text-2xl">{dict.funbox.title}</h1>
        </div>
        <Link href="/games" className="text-xs text-[var(--sky-deep)]">
          ← {dict.games.title}
        </Link>
      </div>
      <FunBox />
    </div>
  );
}
