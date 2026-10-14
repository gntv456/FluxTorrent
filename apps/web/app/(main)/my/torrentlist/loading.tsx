import { getDict } from "@/i18n/server";
import { SkeletonRows } from "@/components/ui/skeleton";

/** /my/torrentlist 路由级加载态（DS §9 验收第 2 条清偿）：我的种子
 *  三 tab 表格是重数据页，RSC 流式入口先出骨架再上数据。 */
export default async function Loading() {
  const { dict } = await getDict();
  const t = dict.mytl.title;
  return (
    <div className="flex flex-col gap-3">
      <h1 className="font-display text-2xl">{t}</h1>
      <SkeletonRows rows={10} />
    </div>
  );
}
