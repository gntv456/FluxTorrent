import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /torrents 路由级加载态（DS-PLAN-01 W-29）：RSC 流式入口——首屏先出
 *  骨架（IA-4：带「正在获取」heading），数据到位后替换，杜绝白屏跳转。
 *  搜索页（?search=）同路由，共用本加载态。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingList}</h1>
      <SkeletonBlock height={56} label={dict.ui.loadingList} />
      <SkeletonRows rows={10} />
    </div>
  );
}
