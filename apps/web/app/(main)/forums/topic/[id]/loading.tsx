import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /forums/topic/[id] 路由级加载态（DS-PLAN-01 W-29）：主题详情骨架
 *  （标题块 + 楼层行）。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingDetail}</h1>
      <SkeletonBlock height={72} label={dict.ui.loadingDetail} />
      <SkeletonRows rows={8} />
    </div>
  );
}
