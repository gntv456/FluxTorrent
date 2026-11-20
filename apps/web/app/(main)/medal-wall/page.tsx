import { getDict } from "@/i18n/server";
import { ErrorPanel } from "@/components/ui/error-panel";
import { normalizeError } from "@/lib/render-error";
import { PageHeader } from "@/components/ui/page-header";
import { getMedalRarities, getMedals } from "@/lib/data";
import { MedalWall } from "@/components/medal-wall";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

/** 勋章墙（medal_wall.php 插件复刻）：全站用户的勋章展示墙。
 *  RSC 侧额外取 /medals 只为拿到「全站勋章种类数」这一准确口径（墙接口不含）。 */
export default async function MedalWallPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("medals");
  if (gate) return gate;

  const { dict } = await getDict();
  // 稀有度词表（0143）：角标配色/标签/筛选档位都由它驱动
  let rarities, medals;
  try {
    [rarities, { items: medals }] = await Promise.all([
      getMedalRarities(),
      getMedals(),
    ]);
  } catch (e) {
    return <ErrorPanel error={normalizeError(e)} retryHref="/medal-wall" />;
  }
  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow="Medal Wall"
        title={dict.medalwall.title}
        sub={dict.medalwall.subtitle}
      />
      <MedalWall rarities={rarities} medalKinds={medals.length} />
    </div>
  );
}
