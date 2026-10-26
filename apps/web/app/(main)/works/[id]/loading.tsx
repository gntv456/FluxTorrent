import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** app/(main)/works/[id] 路由级加载态：门面 + 种子行 + 相关推荐三块
 *  的首屏骨架（数据到位后由 RSC 流式替换）。 */
export default async function Loading() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{dict.works.title}</h1>
      <SkeletonBlock height={192} label={dict.works.title} />
      <SkeletonRows rows={4} />
    </div>
  );
}
