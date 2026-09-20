import { getDict } from "@/i18n/server";
import { getMedalRarities } from "@/lib/data";
import { MedalWall } from "@/components/medal-wall";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 勋章墙（medal_wall.php 插件复刻）：全站用户的勋章展示墙 */
export default async function MedalWallPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("medals");
  if (gate) return gate;

  const { dict } = await getDict();
  // 稀有度词表（0143）：与勋章殿堂同源，角标配色/标签按它渲染
  const rarities = await getMedalRarities();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.medalwall.title}</h1>
      <p className="text-sm text-sub">{dict.medalwall.subtitle}</p>
      <MedalWall rarities={rarities} />
    </div>
  );
}
