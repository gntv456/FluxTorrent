import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /notifications 路由级加载态（DS-PLAN-01 W-29）：通知列表骨架。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingList}</h1>
      <SkeletonBlock height={44} label={dict.ui.loadingList} />
      <SkeletonRows rows={8} />
    </div>
  );
}
