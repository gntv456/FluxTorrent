import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /forums 路由级加载态（DS-PLAN-01 W-29）：板块卡片栅格 + 主题行骨架。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingList}</h1>
      <div className="grid gap-3 md:grid-cols-2">
        <SkeletonBlock height={120} label={dict.ui.loadingList} />
        <SkeletonBlock height={120} label={dict.ui.loadingList} />
      </div>
      <SkeletonRows rows={8} />
    </div>
  );
}
