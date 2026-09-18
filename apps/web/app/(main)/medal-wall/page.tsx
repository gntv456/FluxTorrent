import { getDict } from "@/i18n/server";
import { MedalWall } from "@/components/plugins";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 勋章墙（medal_wall.php 插件复刻）：全站用户的勋章展示墙 */
export default async function MedalWallPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("medals");
  if (gate) return gate;

  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.medalwall.title}</h1>
      <p className="text-sm text-sub">{dict.medalwall.subtitle}</p>
      <MedalWall />
    </div>
  );
}
