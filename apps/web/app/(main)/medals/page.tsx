import { getMedals, getMedalRarities } from "@/lib/data";
import { MedalHall } from "@/components/medal-hall";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 勋章殿堂（好学站 medal.php 口径）：按分类分组，碑卡展示稀有度/获取方式/有效期/
 *  库存/售卖期/加成。交互（分类 Tab/筛选/排序）在客户端组件里做本地筛选，接口不变。 */
export default async function MedalsPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("medals");
  if (gate) return gate;

  const { dict } = await getDict();
  const [medals, rarities] = await Promise.all([
    getMedals(),
    getMedalRarities(),
  ]);
  const owned = medals.filter((m) => m.owned);
  const wearing = medals.find((m) => m.wearing);
  // 「当前加成」= 佩戴中那枚的加成（单佩戴位；拥有但不佩戴不生效）
  const bonus = wearing?.bonus_addition_factor ?? 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Medal Hall</div>
          <h1 className="font-display text-2xl">{dict.medals.title}</h1>
        </div>
        <span className="sub">{dict.medals.subtitle}</span>
        <div className="aside">
          <span className="pill">
            {fmt(dict.medals.mineOwned, {
              n: owned.length,
              total: medals.length,
            })}
          </span>
          {wearing && (
            <span className="pill">
              {dict.medals.mineWearing.replace("{name}", wearing.name)}
            </span>
          )}
          <span className="pill">
            {dict.medals.mineBonus.replace("{n}", bonus.toFixed(2))}
          </span>
        </div>
      </div>
      <MedalHall medals={medals} rarities={rarities} />
    </div>
  );
}
