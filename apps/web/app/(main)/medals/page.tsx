import { getMedals, getMedalRarities } from "@/lib/data";
import { MedalHall } from "@/components/medal-hall";
import { MedalProgress } from "@/components/medal-progress";
import { getDict } from "@/i18n/server";
import { fmt } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";
import { PageHeader } from "@/components/ui/page-header";

export const dynamic = "force-dynamic";

/** 勋章殿堂（好学站 medal.php 口径）：按分类分组，碑卡展示稀有度/获取方式/有效期/
 *  库存/售卖期/加成。交互（分类 Tab/筛选/排序）在客户端组件里做本地筛选，接口不变。
 *  0204：佩戴改多佩戴位——头部摘要「佩戴 n/上限」+ 加成取全部佩戴勋章之和。 */
export default async function MedalsPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("medals");
  if (gate) return gate;

  const { dict } = await getDict();
  const [{ items: medals, max_worn: maxWorn }, rarities] =
    await Promise.all([getMedals(), getMedalRarities()]);
  const owned = medals.filter((m) => m.owned);
  const wearing = medals.filter((m) => m.wearing);
  // 「当前加成」= 佩戴中勋章的加成之和（0204 多佩戴位）
  const bonus = wearing.reduce(
    (acc, m) => acc + m.bonus_addition_factor,
    0,
  );

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow="Medal Hall"
        title={dict.medals.title}
        sub={dict.medals.subtitle}
        aside={
          <>
            <span className="pill">
              {fmt(dict.medals.mineOwned, {
                n: owned.length,
                total: medals.length,
              })}
            </span>
            <span className="pill">
              {fmt(dict.medals.wornCount, {
                n: wearing.length,
                max: maxWorn,
              })}
            </span>
            <span className="pill">
              {dict.medals.mineBonus.replace("{n}", bonus.toFixed(2))}
            </span>
          </>
        }
      />
      <MedalHall medals={medals} rarities={rarities} maxWorn={maxWorn} />
      {/* 0382 自动成就进度：放在殿堂之下，让「已有啥」与「还差啥」
          连着看——两者割裂会让用户以为勋章只能买。 */}
      <section className="flex flex-col gap-3">
        <h2 className="text-sm font-medium text-[var(--text-body)]">
          {dict.medals.title}
        </h2>
        <MedalProgress />
      </section>
    </div>
  );
}
