import { getDict } from "@/i18n/server";
import { SkeletonBlock, SkeletonRows } from "@/components/ui/skeleton";

/** 论坛三栏壳的中栏加载态（首页与版块页共用；DS §9 验收第 2 条）：
 *  骨架落在 (shell)/layout.tsx 的 children 槽里，切版块时左列/右列
 *  保持原位，只有中栏先出骨架（IA-4：带「正在获取」heading）。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.forums.title;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonBlock height={56} label={t} />
      <SkeletonRows rows={10} />
    </div>
  );
}
