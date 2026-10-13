import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /wiki 路由级加载态（DS-PLAN-01 W-29）：目录页与条目页共用——
 *  目录出列表骨架，条目出文块骨架（组合覆盖两种形态）。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingDetail}</h1>
      <SkeletonBlock height={220} label={dict.ui.loadingDetail} />
      <SkeletonRows rows={6} />
    </div>
  );
}
