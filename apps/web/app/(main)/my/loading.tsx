import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** /my 路由级加载态（DS §9 验收第 2 条清偿）：控制面板是重数据页
 *  （四组设定 + 概览），RSC 流式入口先出骨架再上数据。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.my.center;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonBlock height={56} label={t} />
      <SkeletonRows rows={10} />
    </div>
  );
}
