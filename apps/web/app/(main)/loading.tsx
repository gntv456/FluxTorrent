import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** 首页路由级加载态（DS-PLAN-01 W-29）：首页取数多链（home/sticky-promos/
 *  me/overview），RSC 流式窗口期先出区块骨架——海报墙 + 区块行组合，
 *  与 HomeSections 排版形状同构，减小加载完跳动。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.ui.loadingList}</h1>
      <SkeletonBlock height={140} label={dict.ui.loadingList} />
      <div className="grid gap-3 md:grid-cols-2">
        <SkeletonBlock height={180} label={dict.ui.loadingList} />
        <SkeletonBlock height={180} label={dict.ui.loadingList} />
      </div>
      <SkeletonRows rows={6} />
    </div>
  );
}
