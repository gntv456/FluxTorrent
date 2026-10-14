import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** app/(main)/artists 路由级加载态（DS §9 验收第 2 条清偿）：RSC 流式入口——
 *  首屏先出骨架（IA-4：带「正在获取」heading），数据到位后替换。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.artists.title;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonBlock height={56} label={t} />
      <SkeletonRows rows={10} />
    </div>
  );
}
